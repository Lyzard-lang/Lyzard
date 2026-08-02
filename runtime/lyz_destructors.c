// LYZARD Runtime — recursive destructors for compound heap types
#include <stdlib.h>

extern void lyz_release(void* data_ptr);

// LYZARD array layout: [ i64 length | i64 elem_is_refcounted | ptr elements[] ]
// elements[] is an array of `length` pointer-sized slots.
void lyz_release_array_contents(void* data_ptr) {
    long long* header = (long long*)data_ptr;
    long long length             = header[0];
    long long elem_is_refcounted = header[1];

    if (!elem_is_refcounted) return; // e.g. [int] — nothing to recurse into

    void** elements = (void**)((char*)data_ptr + 16); // skip length+flag
    for (long long i = 0; i < length; i++) {
        lyz_release(elements[i]);
    }
}

// LYZARD struct layout descriptors are emitted as compile-time globals:
//   @desc.StructName = global [N x i64] [ count, offset0, offset1, ... ]
// The compiler passes a pointer to the RIGHT descriptor via a hidden
// first field in every struct's data: [ ptr descriptor | field0 | field1 | ... ]
void lyz_release_struct_contents(void* data_ptr) {
    long long* descriptor = *(long long**)data_ptr; // read the hidden descriptor pointer
    if (descriptor == NULL) return;

    long long count = descriptor[0];
    char* fields_start = (char*)data_ptr + 8; // skip the descriptor pointer field

    for (long long i = 0; i < count; i++) {
        long long offset = descriptor[i + 1];
        void* field_ptr = *(void**)(fields_start + offset);
        lyz_release(field_ptr);
    }
}
