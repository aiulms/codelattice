#include <stdio.h>
#include "math_utils.h"
#include "logger.h"

/* Entry point */
int main(void) {
    logger_init("SmokeTest");
    logger_log(LOG_LEVEL_INFO, "Starting smoke test");

    MathResult result;
    result.x = math_add(10, 20);
    result.y = math_subtract(50, 30);

    int product = math_multiply(result.x, result.y);

    printf("Result: x=%d, y=%d, product=%d\n", result.x, result.y, product);

    logger_log(LOG_LEVEL_INFO, "Smoke test complete");
    logger_shutdown();

    return 0;
}
