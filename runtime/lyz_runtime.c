// LYZARD Runtime — minimal C support library
// Linked with every compiled LYZARD program

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// lyz_alloc is implemented in lyz_refcount.c (linked alongside this file).
// Every heap object has a 16-byte header: [ i64 refcount | i64 type_tag ]
#define LYZ_TAG_STR 0
extern void* lyz_alloc(long long size, long long type_tag);

void lyz_print_int(long long n) {
    printf("%lld", n);
}

void lyz_print_float(double f) {
    // Match LYZARD's float display: whole numbers show one decimal
    if (f == (long long)f) {
        printf("%.1f", f);
    } else {
        printf("%g", f);
    }
}

void lyz_print_bool(int b) {
    printf(b ? "true" : "false");
}

void lyz_print_str(const char* s) {
    printf("%s", s);
}

void lyz_println(void) {
    printf("\n");
}

// Simple string concatenation helper — returns a newly malloc'd string
char* lyz_str_concat(const char* a, const char* b) {
    size_t len_a = strlen(a);
    size_t len_b = strlen(b);
    char* result = (char*)malloc(len_a + len_b + 1);
    memcpy(result, a, len_a);
    memcpy(result + len_a, b, len_b + 1); // +1 copies null terminator
    return result;
}

// Slice a string from start (inclusive) to end (exclusive), indexing by
// character. Out-of-range bounds are clamped; a reversed range returns "".
// Returns a newly allocated (refcounted) string.
char* lyz_slice(const char* s, long long start, long long end) {
    size_t len = strlen(s);
    if (start < 0) start = 0;
    if (end > (long long)len) end = (long long)len;
    if (end < start) {
        char* empty = (char*)lyz_alloc(1, LYZ_TAG_STR);
        empty[0] = '\0';
        return empty;
    }
    size_t sub_len = (size_t)(end - start);
    char* result = (char*)lyz_alloc(sub_len + 1, LYZ_TAG_STR);
    memcpy(result, s + start, sub_len);
    result[sub_len] = '\0';
    return result;
}

// Length of a string (in bytes) — maps the builtin `len` for strings
long long lyz_strlen(const char* s) {
    return (long long)strlen(s);
}

// Panic — print error and exit
void lyz_panic(const char* message) {
    fprintf(stderr, "\n🦎 LYZARD Panic: %s\n", message);
    exit(1);
}

// Division by zero check (called before every int division)
void lyz_check_div_zero(long long divisor) {
    if (divisor == 0) {
        lyz_panic("division by zero");
    }
}

// Array bounds check
void lyz_check_bounds(long long index, long long length) {
    if (index < 0 || index >= length) {
        fprintf(stderr, "\n🦎 LYZARD Panic: index %lld out of bounds (length %lld)\n", index, length);
        exit(1);
    }
}
