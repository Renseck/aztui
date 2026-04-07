## Coding styleguide

### File structure

#### Include Statements
- Place all use statements at the top of files

### Separators

Between code, try to use separators in a way that corresponds to the level of difference between what's being separated. Maximum separator length should be kept to a maximum length of 100, and indented according to the level of code that's being separated.
The VSCode extension [Comment Divider](https://marketplace.visualstudio.com/items?itemName=stackbreak.comment-divider) can be used to this neatly.

- Functions within the same type are separated with:
  ```rs
  /* ============================================================================================== */
  ```
  These are also used to separate attributes from methods in class definitions.
- Function categories can be delineated with:
  ```rs
  /* ========================================== Category ========================================== */
  ```

- Larger banner may be used to serve as clearer separations of concerns, e.g. for separating public, protected and private functions
  ```rs
  /* ============================================================================================== */
  /*                                        Protected methods                                       */
  /* ============================================================================================== */
  ```
Separators after a previous function definitions should keep one empty line after the function definition end. Function definitions (or the docstring right above them) should be started directly after a separator line. Example:
```rs
fn foo() {

}

// ============================================================================================== //
/**
 * Documentation
 */
fn bar() {

}
```

### Indentation

- Try to stick to using K&R style indentation, which keeps the opening curly bracket on the same line as the statement it belongs to:
  ```rs
  // Function definitions
  function foo(arg1: String, arg2: usize) -> bool {
    // Do something
  }

  ```
- Tabs are 4 spaces long;

### Naming conventions

#### Variables and Functions
- Use snake_case for variables, function names: `article_id`, `get_user_by_id()`.
- Use PascalCase for class names: `ArticleModel`.
- Use descriptive names that clearly indicate purpose

### Error Handling

- Use Result type

### Documentation

#### Function Documentation
- Use rustdoc format
- Include parameter types and return types
- Add brief description of functionality

Example:
```rs
impl Person {
    /// Creates a person with the given name.
    ///
    /// # Examples
    ///
    /// ```
    /// // You can have rust code between fences inside the comments
    /// // If you pass --test to `rustdoc`, it will even test it for you!
    /// use doc::Person;
    /// let person = Person::new("name");
    /// ```
    pub fn new(name: &str) -> Person {
        Person {
            name: name.to_string(),
        }
    }
```

### Comments

- Use single-line comments (`//`) for brief explanations
- Use multi-line comments (`/* */`) for longer explanations
- Avoid obvious comments; focus on explaining "why" not "what"