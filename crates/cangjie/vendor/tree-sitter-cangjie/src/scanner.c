#include <tree_sitter/parser.h>
#include <wctype.h>
#include <string.h>
#include <stdio.h>

// 定义一个宏来控制是否启用调试
#define DEBUG_SCANNER 0

enum TokenType {
  _MULTI_LINE_RAW_STRING_START,
  _MULTI_LINE_RAW_STRING_CONTENT,
  _MULTI_LINE_RAW_STRING_END,
};

typedef struct {
  bool in_string;            // Whether we're currently inside a string
  uint8_t delimiter_length;  // Number of '#' characters in current delimiter
} Scanner;

static FILE* log_file = NULL;
static void logger(TSLexer* lexer, const char* msg) {
  if (! DEBUG_SCANNER) return;
  if (log_file == NULL) {
    fprintf(stdout, msg);
    return;
  }
  fprintf(log_file, msg);
  fflush(log_file);
}

// ----------------------------------------------------------
// Utility Functions
// ----------------------------------------------------------

static void advance(TSLexer *lexer) {
  lexer->advance(lexer, false);
}

static void skip(TSLexer *lexer) {
  lexer->advance(lexer, true);
}

// ----------------------------------------------------------
// Scanner Lifecycle Functions
// ----------------------------------------------------------

void *tree_sitter_cangjie_external_scanner_create() {
  if (DEBUG_SCANNER) {
    log_file = fopen("/home/yxm/tree-sitter-cangjie.log", "wa+");
  }
  Scanner *scanner = calloc(1, sizeof(Scanner));
  scanner->delimiter_length = 0;
  scanner->in_string = false;
  return scanner;
}

void tree_sitter_cangjie_external_scanner_destroy(void *payload) {
  if (log_file != NULL) {
    fclose(log_file);
    log_file = NULL;
  }
  free(payload);
}

unsigned tree_sitter_cangjie_external_scanner_serialize(void *payload, char *buffer) {
  Scanner *scanner = (Scanner *)payload;
  buffer[0] = scanner->in_string? 1: 0;
  buffer[1] = (char)scanner->delimiter_length;
  return 2;
}

void tree_sitter_cangjie_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  Scanner *scanner = (Scanner *)payload;
  if (length >= 2) {
    scanner->in_string = buffer[0];
    scanner->delimiter_length = (uint8_t)buffer[1];
  } else {
    scanner->delimiter_length = 0;
    scanner->in_string = false;
  }
}

// ----------------------------------------------------------
// Delimiter Scanning Functions
// ----------------------------------------------------------

static bool scan_opening_delimiter(TSLexer *lexer, Scanner *scanner) {
  // Count the number of '#' characters
  uint8_t hash_count = 0;
  while (lexer->lookahead == '#') {
    advance(lexer);
    hash_count++;
    if (hash_count == UINT8_MAX) return false;  // Prevent overflow
  }

  // Must be followed by a quote
  if (lexer->lookahead != '"' || hash_count == 0) {
    scanner->delimiter_length = 0;
    scanner->in_string = false;
    return false;
  }

  advance(lexer);

  // Store the delimiter length and mark as in string
  scanner->delimiter_length = hash_count;
  scanner->in_string = true;
  lexer->result_symbol = _MULTI_LINE_RAW_STRING_START;
  return true;
}

static bool scan_closing_delimiter(TSLexer *lexer, Scanner *scanner) {  
//  DEBUG_PRINT("Scanning opening delimiter, current char: '%c'\n", lexer->lookahead);

  advance(lexer);
  
  // Count the number of '#' characters
  uint8_t hash_count = 0;
  while (lexer->lookahead == '#') {
    advance(lexer);
    hash_count++;
    if (hash_count == UINT8_MAX) return false;
  }

  // Must be followed by a quote and match opening delimiter length
  if (hash_count != scanner->delimiter_length) {
    return false;
  }
  advance(lexer);

  // Reset scanner state
  scanner->delimiter_length = 0;
  scanner->in_string = false;
  lexer->result_symbol = _MULTI_LINE_RAW_STRING_END;
  return true;
}

// ----------------------------------------------------------
// Content Scanning Function
// ----------------------------------------------------------

static bool scan_string_content(TSLexer *lexer, Scanner *scanner) {
  if (!scanner->in_string) return false;

  lexer->result_symbol = _MULTI_LINE_RAW_STRING_CONTENT;

  while (true) {
    // Check for potential closing delimiter
    if (lexer->lookahead == '"') {
      lexer->mark_end(lexer); //标记内容的结束位置
      uint8_t hash_count = 0;
      advance(lexer);
      // Count the '#' characters
      while (lexer->lookahead == '#') {
        advance(lexer);
        hash_count++;
      }
      // Check if it's a valid closing delimiter
      if (hash_count == scanner->delimiter_length) {
        // Not part of the content - return what we have
        return true;
      }
    }
    // Handle EOF case
    else if (lexer->lookahead == 0) {
      lexer->mark_end(lexer);
      return true;
    } 
    // Normal content character
    else {
      advance(lexer);
    }
  }
}

// ----------------------------------------------------------
// Main Scanning Function
// ----------------------------------------------------------

bool tree_sitter_cangjie_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  Scanner *scanner = (Scanner *)payload;

  // Skip whitespace (only relevant for opening delimiter)
  while (iswspace(lexer->lookahead)) {
    skip(lexer);
  }

  // Check for delimiters first
  if (valid_symbols[_MULTI_LINE_RAW_STRING_START] && !scanner->in_string && lexer->lookahead=='#') {
    return scan_opening_delimiter(lexer, scanner);
  }
  // Then check for string content
  if (valid_symbols[_MULTI_LINE_RAW_STRING_CONTENT] && scanner->in_string) {
    return scan_string_content(lexer, scanner);
  }
  // Looking for closing delimiter
  if (valid_symbols[_MULTI_LINE_RAW_STRING_END] && scanner->in_string && lexer->lookahead=='"') {
    return scan_closing_delimiter(lexer, scanner);
  }

  return false;
}