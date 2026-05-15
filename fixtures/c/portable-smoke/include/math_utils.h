#ifndef MATH_UTILS_H
#define MATH_UTILS_H

/* Function declarations */
int math_add(int a, int b);
int math_subtract(int a, int b);
int math_multiply(int a, int b);

/* Struct definition */
typedef struct {
    int x;
    int y;
} MathResult;

/* Enum definition */
typedef enum {
    MATH_OK = 0,
    MATH_ERROR_OVERFLOW = 1,
    MATH_ERROR_DIVISION_BY_ZERO = 2,
} MathStatus;

/* Typedef (function pointer — Phase A: record typedef, no pointer resolution) */
typedef int (*MathOp)(int, int);

#endif /* MATH_UTILS_H */
