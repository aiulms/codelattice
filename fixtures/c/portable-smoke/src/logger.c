#include <stdio.h>
#include <string.h>
#include "logger.h"

/* Global variable */
static int g_initialized = 0;
static char g_app_name[128];

void logger_init(const char *app_name) {
    strncpy(g_app_name, app_name, sizeof(g_app_name) - 1);
    g_app_name[sizeof(g_app_name) - 1] = '\0';
    g_initialized = 1;
}

void logger_log(int level, const char *message) {
    if (!g_initialized) return;
    const char *level_str = "UNKNOWN";
    if (level == LOG_LEVEL_DEBUG) level_str = "DEBUG";
    if (level == LOG_LEVEL_INFO) level_str = "INFO";
    if (level == LOG_LEVEL_ERROR) level_str = "ERROR";
    printf("[%s] %s: %s\n", g_app_name, level_str, message);
}

void logger_shutdown(void) {
    g_initialized = 0;
    g_app_name[0] = '\0';
}
