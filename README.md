Add parameters, generics and return types to all functions in the impl block

# Examples

**Add parameters**

```rust
struct Foo(i32);

#[using_param::using_param(&self, other: &Foo)]
#[using_param::using_return(bool)]
impl PartialEq for Foo {
    fn eq() { self.0 == other.0 }
    fn ne() { self.0 != other.0 }
}
assert!(Foo(2) == Foo(2));
assert!(Foo(2) != Foo(3));
```

**Default self parameter**

```rust
struct Foo(i32);

#[using_param::using_param(self)]
impl Foo {
    fn use_ref(&self) -> i32 { self.0 }
    fn use_owned() -> i32 { self.0 }
}

let foo = Foo(3);
assert_eq!(foo.use_ref(), 3);
assert_eq!(foo.use_owned(), 3);
```

**Default return type**

```rust
struct Foo(i32);

#[using_param::using_return(i32)]
impl Foo {
    fn use_ref(&self) -> i32 { self.0 }
    fn use_owned(self) { self.0 }
}

let foo = Foo(3);
assert_eq!(foo.use_ref(), 3);
assert_eq!(foo.use_owned(), 3);
```

**Add parameters before all parameters**

```rust
struct Foo(f64);

#[using_param::using_param(other: Foo)]
impl Foo {
    fn div_with(this: Foo) -> f64 { this.0 / other.0 }
}

assert_eq!(Foo::div_with(Foo(8.0), Foo(4.0)), 0.5);
```

**Add parameters after all parameters**

```rust
struct Foo(f64);

#[using_param::using_param(, other: Foo)]
impl Foo {
    fn div(this: Foo) -> f64 { this.0 / other.0 }
}

assert_eq!(Foo::div(Foo(8.0), Foo(4.0)), 2.0);
```

**Add generic parameters**

```rust
struct Foo;

#[using_param::using_generic(, U: Default)]
impl Foo {
    fn foo() -> U { U::default() }
    fn bar<T: Default>() -> (T, U) { (T::default(), U::default()) }
}

assert_eq!(Foo::foo::<i32>(), 0);
assert_eq!(Foo::bar::<i32, &str>(), (0, ""));
```
