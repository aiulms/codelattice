#ifndef MATH_UTILS_HPP
#define MATH_UTILS_HPP

#include <cmath>

namespace utils {

class MathUtils {
public:
    static double add(double a, double b);
    static double multiply(double a, double b);

    double compute(int x) const;

private:
    double factor_ = 1.0;
};

struct Point {
    double x;
    double y;

    double distance() const;
};

enum class Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
};

double apply_operation(Operation op, double a, double b);

using MathFunc = double(*)(double, double);

} // namespace utils

#endif // MATH_UTILS_HPP
