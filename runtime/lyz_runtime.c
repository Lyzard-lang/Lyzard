// LYZARD Runtime — minimal C support library
// Linked with every compiled LYZARD program

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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
