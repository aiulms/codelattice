#include "math_utils.hpp"
#include "logger.hpp"
#include <iostream>

using namespace utils;

int main() {
    Logger logger(LogLevel::Info);
    logger.log("Starting math operations");

    double sum = MathUtils::add(3.14, 2.72);
    double product = MathUtils::multiply(4.0, 5.0);

    std::cout << "Sum: " << sum << std::endl;
    std::cout << "Product: " << product << std::endl;

    Point p{3.0, 4.0};
    double dist = p.distance();
    std::cout << "Distance: " << dist << std::endl;

    Operation op = Operation::Add;
    double result = apply_operation(op, 10.0, 20.0);
    std::cout << "Apply operation: " << result << std::endl;

    MathUtils math_obj;
    double computed = math_obj.compute(42);
    std::cout << "Computed: " << computed << std::endl;

    initialize_logging();

    return 0;
}
