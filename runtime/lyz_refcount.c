// LYZARD Runtime — Reference counting support
// Every heap object has a 16-byte header: [ i64 refcount | i64 type_tag ]
// immediately before the data pointer that LYZARD code holds.

#include <stdlib.h>
#include <stdio.h>

#define LYZ_HEADER_SIZE 16
#define LYZ_TAG_STR    0
#define LYZ_TAG_ARRAY  1
#define LYZ_TAG_STRUCT 2

typedef struct {
    long long refcount;
    long long type_tag;
} LyzHeader;

static LyzHeader* lyz_header_of(void* data_ptr) {
    return (LyzHeader*)((char*)data_ptr - LYZ_HEADER_SIZE);
}

// Allocate `size` bytes of data plus the header. Returns a pointer to
// the data (header is hidden immediately before it).
void* lyz_alloc(long long size, long long type_tag) {
    void* raw = malloc(LYZ_HEADER_SIZE + size);
    if (!raw) {
        fprintf(stderr, "\n🦎 LYZARD Panic: out of memory\n");
        exit(1);
    }
    LyzHeader* header = (LyzHeader*)raw;
    header->refcount = 1;
    header->type_tag = type_tag;
    return (char*)raw + LYZ_HEADER_SIZE;
}

// Increment the refcount — called whenever a reference is copied
// (assignment, passed as function argument, stored into a struct field)
void lyz_retain(void* data_ptr) {
    if (data_ptr == NULL) return;
    LyzHeader* header = lyz_header_of(data_ptr);
    header->refcount += 1;
}

// Forward declaration — the array/struct destructors need to recursively
// release nested pointers before freeing themselves. Their real recursive
// implementations live in lyz_destructors.c (linked alongside this file).
void lyz_release_array_contents(void* data_ptr);
void lyz_release_struct_contents(void* data_ptr);

// Decrement the refcount — called whenever a reference goes out of scope.
// Frees the object (and recursively releases its children) if it hits 0.
void lyz_release(void* data_ptr) {
    if (data_ptr == NULL) return;
    LyzHeader* header = lyz_header_of(data_ptr);
    header->refcount -= 1;

    if (header->refcount <= 0) {
        // Recursively release any nested heap references before freeing
        switch (header->type_tag) {
            case LYZ_TAG_ARRAY:
                lyz_release_array_contents(data_ptr);
                break;
            case LYZ_TAG_STRUCT:
                lyz_release_struct_contents(data_ptr);
                break;
            case LYZ_TAG_STR:
            default:
                break; // strings have no nested references
        }
        free(header); // frees header + data in one call (contiguous allocation)
    }
}

// Debug helper — check current refcount without modifying it
long long lyz_refcount_of(void* data_ptr) {
    if (data_ptr == NULL) return 0;
    return lyz_header_of(data_ptr)->refcount;
}
