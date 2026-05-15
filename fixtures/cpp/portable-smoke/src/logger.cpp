#include "logger.hpp"
#include <iostream>

namespace utils {

bool Logger::enabled_ = true;

Logger::Logger() : level_(LogLevel::Info) {}

Logger::Logger(LogLevel level) : level_(level) {}

Logger::~Logger() {
    log("Logger destroyed");
}

void Logger::log(const std::string& message) {
    if (!enabled_) return;
    std::cout << "[" << static_cast<int>(level_) << "] " << message << std::endl;
}

void Logger::set_level(LogLevel level) {
    level_ = level;
}

void Logger::enable() {
    enabled_ = true;
}

void Logger::disable() {
    enabled_ = false;
}

void initialize_logging() {
    Logger::enable();
    Logger logger(LogLevel::Debug);
    logger.log("Logging initialized");
}

} // namespace utils
