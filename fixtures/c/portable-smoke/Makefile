CC = cc
CFLAGS = -Wall -Wextra -std=c99 -Iinclude

SRCS = src/main.c src/math_utils.c src/logger.c
OBJS = $(SRCS:.c=.o)
TARGET = smoke_test

all: $(TARGET)

$(TARGET): $(OBJS)
	$(CC) $(CFLAGS) -o $@ $^

%.o: %.c
	$(CC) $(CFLAGS) -c -o $@ $<

clean:
	rm -f $(OBJS) $(TARGET)

.PHONY: all clean
