#include <stdio.h>
#include <stdlib.h>
#include "math_utils.h"
#include "logger.h"

/* Global variable */
static int g_call_count = 0;

/* Function definitions */
int math_add(int a, int b) {
    g_call_count++;
    return a + b;
}

int math_subtract(int a, int b) {
    g_call_count++;
    return a - b;
}

int math_multiply(int a, int b) {
    g_call_count++;
    return a * b;
}

/* Static (file-local) helper function */
static int clamp_result(int value, int min_val, int max_val) {
    if (value < min_val) return min_val;
    if (value > max_val) return max_val;
    return value;
}
