#include "math_utils.hpp"
#include "logger.hpp"
#include <algorithm>

namespace utils {

double MathUtils::add(double a, double b) {
    return a + b;
}

double MathUtils::multiply(double a, double b) {
    return a * b;
}

double MathUtils::compute(int x) const {
    return static_cast<double>(x) * factor_;
}

double Point::distance() const {
    return std::sqrt(x * x + y * y);
}

double apply_operation(Operation op, double a, double b) {
    switch (op) {
        case Operation::Add:
            return MathUtils::add(a, b);
        case Operation::Subtract:
            return a - b;
        case Operation::Multiply:
            return MathUtils::multiply(a, b);
        case Operation::Divide:
            return b != 0.0 ? a / b : 0.0;
    }
    return 0.0;
}

namespace {
    double internal_helper(double val) {
        return val * 2.0;
    }
}

} // namespace utils
