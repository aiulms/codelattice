#ifndef LOGGER_H
#define LOGGER_H

/* Macro definition */
#define LOG_LEVEL_DEBUG 0
#define LOG_LEVEL_INFO 1
#define LOG_LEVEL_ERROR 2

#define MAX_LOG_MSG 256

/* Function declarations */
void logger_init(const char *app_name);
void logger_log(int level, const char *message);
void logger_shutdown(void);

#endif /* LOGGER_H */
