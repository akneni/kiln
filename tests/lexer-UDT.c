#include <stdio.h>

// Test case 1: Simple struct definition
struct Point {
    int x;
    int y;
};

// Test case 2: Typedef struct without a tag
typedef struct {
    float real;
    float imag;
} Complex;

// Test case 3: Typedef struct with a tag
typedef struct Rectangle {
    int width;
    int height;
} Rectangle;

// Test case 4: Struct with a single member
struct Circle {
    float radius;
};

// Test case 5: Struct with a nested struct definition
struct Line {
    // Nested struct definition inside a struct
    struct Point {
        int x;
        int y;
    } start, end;
};

// Test case 6: Struct with an anonymous struct member
struct Node {
    int value;
    struct {
        int left;
        int right;
    } children;
};

// Test case 7: Struct representing a linked list node (self-referential)
struct List {
    int data;
    struct List *next;
};

// Test case 8: Struct with bit fields
struct Flags {
    unsigned int flag1 : 1;
    unsigned int flag2 : 1;
};

// Test case 9: Typedef struct with more complex members
typedef struct Employee {
    char name[50];
    int id;
    float salary;
} Employee;

// Test case 10: Struct for a binary tree node (self-referential pointers)
struct TreeNode {
    int value;
    struct TreeNode *left;
    struct TreeNode *right;
};

// Test case 11: Typedef struct with a different alias
typedef struct Car {
    int wheels;
    float engine_power;
} Vehicle;

// Test case 12: Simple enum definition with a tag
enum Color {
    RED,
    GREEN,
    BLUE
};

// Test case 13: Typedef enum without a tag
typedef enum {
    CIRCLE,
    SQUARE,
    TRIANGLE
} Shape;

// Test case 14: Enum with explicit values
enum Direction {
    NORTH = 0,
    EAST = 90,
    SOUTH = 180,
    WEST = 270
};

// Test case 15: Typedef enum with a tag
typedef enum Day {
    MONDAY,
    TUESDAY,
    WEDNESDAY,
    THURSDAY,
    FRIDAY,
    SATURDAY,
    SUNDAY
} Day;

// Test case 16: Simple union definition with a tag
union Data {
    int i;
    float f;
    char *s;
};

// Test case 17: Typedef union without a tag
typedef union {
    int i;
    float f;
    char *s;
} Data;

// Test case 18: Union with a nested struct
union Mixed {
    struct {
        int a;
        int b;
    } pair;
    float f;
};

// Test case 19: Anonymous union within a struct (C11 or as a compiler extension)
struct Container {
    int id;
    union {
        int i;
        float f;
    };  // Note: This union is anonymous and its members become members of Container.
};

/* Test Case 20: Random bullshit that doesn't work for some reason */
typedef enum BlockType {
	BlockTypeCreate,
	BlockTypeUpdate,
	BlockTypeDelete,
} BlockType;

/* Test Case 21: Struct with additional attributes */
typedef struct TcpPacket {
    unsigned char data[1024];
} TcpPacket __attribute__((aligned(4)));

// Test case 22: Struct with function pointer members
struct Callbacks {
    void (*onStart)(void);
    int (*calculate)(int, int);
    void (*cleanup)(void*);
};

// Test case 23: Forward declaration of a struct
struct ForwardDeclared;

// Test case 24: Typedef for a function pointer type
typedef int (*CompareFn)(const void*, const void*);

// Test case 25: Struct with a flexible array member (C99 feature)
struct Buffer {
    int size;
    char data[];  // Flexible array member
};

// Test case 26: Struct with multiple nested comments
struct /* outer comment start */ CommentTest /* between comments */ {
    int /* comment before member */ value; /* comment after member */
} /* comment before semicolon */;

// Test case 27: Nested typedef declarations
typedef struct {
    typedef enum {
        INNER_ONE,
        INNER_TWO
    } InnerEnum;
    InnerEnum value;
} OuterStruct;

// Test case 28: Union with bit fields
union BitAccess {
    unsigned int fullValue;
    struct {
        unsigned int lowBits : 16;
        unsigned int highBits : 16;
    } parts;
};

// Test case 29: Empty struct declaration
struct Empty {
};

// Test case 30: Multi-dimensional array member in a struct
struct Matrix {
    float elements[4][4];
    char name[32];
};

// Test case 31: Typedef for a void pointer
typedef void* VoidPtr;

// Test case 32: Typedef for an array type
typedef int IntArray[10];

// Test case 33: Union with anonymous enum
union WithEnum {
    enum {
        STATE_IDLE,
        STATE_BUSY,
        STATE_ERROR
    } state;
    int rawState;
};

// Test case 34: Struct with volatile and const members
struct SpecialMembers {
    volatile int counter;
    const char* message;
    char* const buffer;
};

// Test case 35: Complex typedef with function pointer returning a struct pointer
typedef struct Node* (*NodeFactory)(int value);
