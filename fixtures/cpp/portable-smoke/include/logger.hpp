#ifndef LOGGER_HPP
#define LOGGER_HPP

#include <string>

namespace utils {

enum LogLevel {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
};

class Logger {
public:
    Logger();
    explicit Logger(LogLevel level);
    ~Logger();

    void log(const std::string& message);
    void set_level(LogLevel level);

    static void enable();
    static void disable();

private:
    LogLevel level_;
    static bool enabled_;
};

void initialize_logging();

} // namespace utils

#endif // LOGGER_HPP
