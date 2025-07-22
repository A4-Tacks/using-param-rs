#![doc = include_str!("../README.md")]

use proc_macro::*;
use proc_macro::Spacing::*;
use proc_macro_tool::*;

#[proc_macro_attribute]
pub fn using_param(attr: TokenStream, item: TokenStream) -> TokenStream {
    process(Type::Param, attr, item)
}

#[proc_macro_attribute]
pub fn using_generic(attr: TokenStream, item: TokenStream) -> TokenStream {
    process(Type::Generic, attr, item)
}

#[proc_macro_attribute]
pub fn using_return(attr: TokenStream, item: TokenStream) -> TokenStream {
    process(Type::Ret, attr, item)
}

#[derive(Debug, Clone, Copy)]
enum Type { Param, Generic, Ret }

fn process(ty: Type, attr: TokenStream, item: TokenStream) -> TokenStream {
    let cfg = match ty {
        Type::Param => param_cfg(attr),
        Type::Generic => generic_cfg(attr),
        Type::Ret => Conf { return_type: attr, ..Default::default() },
    };

    let mut iter = item.parse_iter();
    let mut out = TokenStream::new();
    out.extend(iter.next_attributes());

    if !iter.peek_is(|i| i.is_keyword("impl")) {
        err!("expected impl keyword", iter.span())
    }
    let block = iter.reduce(|a, b| {
        out.push(a);
        b
    }).unwrap();

    let items = block.to_brace_stream().unwrap();
    match process_impl_block(&cfg, items) {
        Err(e) => e,
        Ok(b) => {
            out.push(b.grouped_brace().tt());
            out
        }
    }
}

#[derive(Debug, Default)]
struct Conf {
    params_after: bool,
    param: TokenStream,
    generics: TokenStream,
    generics_after: bool,
    return_type: TokenStream,
}

fn fn_generic(iter: &mut ParseIter<impl Iterator<Item = TokenTree>>) -> TokenStream {
    let mut out = TokenStream::new();

    loop {
        if let Some(arrow) = iter.next_puncts("->") {
            out.extend(arrow);
        } else if iter.is_puncts(">")
            && iter.peek_i(1).is_none_or(|t| t.is_delimiter_paren())
        {
            break;
        } else if let Some(tt) = iter.next() {
            out.push(tt);
        } else {
            break;
        }
    }

    out
}

fn param_cfg(attr: TokenStream) -> Conf {
    let mut iter = attr.parse_iter();

    Conf {
        params_after: iter.next_if(|t| t.is_punch(',')).is_some(),
        param: iter.collect(),
        ..Default::default()
    }
}

fn generic_cfg(attr: TokenStream) -> Conf {
    let mut iter = attr.parse_iter();

    Conf {
        generics_after: iter.next_if(|t| t.is_punch(',')).is_some(),
        generics: iter.collect(),
        ..Default::default()
    }
}

fn join_with_comma(a: TokenStream, b: TokenStream) -> TokenStream {
    if a.is_empty() {
        return b;
    }
    if b.is_empty() {
        return a;
    }

    let mut left = a.into_iter().collect::<Vec<_>>();
    let right = b.into_iter().collect::<Vec<_>>();

    left.pop_if(|t| t.is_punch(','));

    if !left.is_empty() {
        left.push(','.punct(Alone).tt());
    }

    let rcom = right.first().is_some_and(|t| t.is_punch(','));
    stream(left.into_iter().chain(right.into_iter().skip(rcom.into())))
}

fn self_param(iter: &mut ParseIter<impl Iterator<Item = TokenTree>>) -> TokenStream {
    macro_rules! ok {
        () => {
            return iter
                .split_puncts_include(",")
                .unwrap_or_else(|| iter.collect());
        };
    }
    if iter.peek_is(|t| t.is_punch('&')) {
        if iter.peek_i_is(1, |t| t.is_punch('\''))
        && iter.peek_i_is(2, |t| t.is_ident())
        {
            if iter.peek_i_is(3, |t| t.is_keyword("self")) {
                ok!();
            }

            if iter.peek_i_is(3, |t| t.is_keyword("mut"))
            && iter.peek_i_is(4, |t| t.is_keyword("self"))
            {
                ok!();
            }
        }

        if iter.peek_i_is(1, |t| t.is_keyword("self")) {
            ok!();
        }

        if iter.peek_i_is(1, |t| t.is_keyword("mut"))
        && iter.peek_i_is(2, |t| t.is_keyword("self"))
        {
            ok!();
        }
    } else if iter.peek_is(|t| t.is_keyword("self")) {
        ok!();
    }
    TokenStream::new()
}

fn process_impl_block(
    cfg: &Conf,
    items: TokenStream,
) -> Result<TokenStream, TokenStream> {
    let mut out = TokenStream::new();
    let mut iter = items.parse_iter();

    out.extend(iter.next_outer_attributes());

    while iter.peek().is_some() {
        out.extend(iter.next_attributes());
        out.extend(iter.next_vis());

        if iter.peek_is(|t| t.is_keyword("fn"))
            && iter.peek_i_is(1, |t| t.is_ident())
        {
            out.extend(iter.next_tts::<2>());

            if iter.push_if_to(&mut out, |t| t.is_punch('<')) {
                let generic = fn_generic(&mut iter);

                out.add(if cfg.generics_after {
                    join_with_comma(generic, cfg.generics.clone())
                } else {
                    join_with_comma(cfg.generics.clone(), generic)
                });

                iter.push_if_to(&mut out, |t| t.is_punch('>'));
            } else if !cfg.generics.is_empty() {
                out.push('<'.punct(Alone).tt());
                out.add(cfg.generics.clone());
                out.push('>'.punct(Alone).tt());
            }

            if let Some(TokenTree::Group(paren))
                = iter.next_if(|t| t.is_delimiter_paren())
            {

                let params_group = paren.map(|paren| {
                    let mut iter = paren.parse_iter();
                    let mut self_ = self_param(&mut iter);
                    let mut param = cfg.param.clone().parse_iter();
                    let other_self = self_param(&mut param);

                    if self_.is_empty() {
                        self_ = other_self;
                    }

                    join_with_comma(self_, if cfg.params_after {
                        join_with_comma(iter.collect(), param.collect())
                    } else {
                        join_with_comma(param.collect(), iter.collect())
                    })
                });
                out.push(params_group.tt());

                if !cfg.return_type.is_empty() && !iter.is_puncts("->") {
                    out.push('-'.punct(Joint).tt());
                    out.push('>'.punct(Alone).tt());
                    out.add(cfg.return_type.clone());
                }
            }
        } else {
            out.push(iter.next().unwrap());
        }
    }

    Ok(out)
}

/// ```
/// using_param::__test_join! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_join(_: TokenStream) -> TokenStream {
    let datas = [
        ("", "", ""),
        ("a", "", "a"),
        ("a,", "", "a,"),
        ("", "a", "a"),
        ("", "a,", "a,"),
        ("a", "b", "a, b"),
        ("a,", "b", "a, b"),
        ("a,", "b,", "a, b,"),
        ("a,", "b,", "a, b,"),
        ("a", "b,", "a, b,"),
    ];
    for (a, b, expected) in datas {
        let out = join_with_comma(a.parse().unwrap(), b.parse().unwrap());
        assert_eq!(out.to_string(), expected, "{a:?}, {b:?}");
    }
    TokenStream::new()
}

/// ```
/// using_param::__test_before! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_before(_: TokenStream) -> TokenStream {
    let out = using_param("ctx: i32".parse().unwrap(), "
impl Foo {
    #[doc(hidden)]
    pub fn foo(&self, s: &str) -> &str {
        s
    }
    pub fn bar(&self) -> i32 {
        ctx
    }
    pub fn baz() -> i32 {
        ctx
    }
    pub fn f(self: &Self) -> i32 {
        ctx
    }
    pub fn a(x: i32) -> i32 {
        ctx+x
    }
    pub fn b(&mut self, a: i32, b: i32) -> i32 {
        ctx+a+b
    }
    pub fn c(&'a mut self, a: i32, b: i32) -> i32 {
        ctx+a+b
    }
    pub fn d(&'static mut self, a: i32, b: i32) -> i32 {
        ctx+a+b
    }
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    #[doc(hidden)]
    pub fn foo(& self, ctx : i32, s : & str) -> & str {
        s
    }
    pub fn bar(& self, ctx : i32) -> i32 {
        ctx
    }
    pub fn baz(ctx : i32) -> i32 {
        ctx
    }
    pub fn f(self : & Self, ctx : i32) -> i32 {
        ctx
    }
    pub fn a(ctx : i32, x : i32) -> i32 {
        ctx+x
    }
    pub fn b(& mut self, ctx : i32, a : i32, b : i32) -> i32 {
        ctx+a+b
    }
    pub fn c(& 'a mut self, ctx : i32, a : i32, b : i32) -> i32 {
        ctx+a+b
    }
    pub fn d(& 'static mut self, ctx : i32, a : i32, b : i32) -> i32 {
        ctx+a+b
    }
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}


/// ```
/// using_param::__test_after! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_after(_: TokenStream) -> TokenStream {
    let out = using_param(", ctx: i32".parse().unwrap(), "
impl Foo {
    #[doc(hidden)]
    pub fn foo(&self, s: &str) -> &str {
        s
    }
    pub fn bar(&self) -> i32 {
        ctx
    }
    pub fn baz() -> i32 {
        ctx
    }
    pub fn f(self: &Self) -> i32 {
        ctx
    }
    pub fn a(x: i32) -> i32 {
        ctx+x
    }
    pub fn b(&mut self, a: i32, b: i32) -> i32 {
        ctx+a+b
    }
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    #[doc(hidden)]
    pub fn foo(& self, s : & str, ctx : i32) -> & str {
        s
    }
    pub fn bar(& self, ctx : i32) -> i32 {
        ctx
    }
    pub fn baz(ctx : i32) -> i32 {
        ctx
    }
    pub fn f(self : & Self, ctx : i32) -> i32 {
        ctx
    }
    pub fn a(x : i32, ctx : i32) -> i32 {
        ctx+x
    }
    pub fn b(& mut self, a : i32, b : i32, ctx : i32) -> i32 {
        ctx+a+b
    }
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}


/// ```
/// using_param::__test_self_param! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_self_param(_: TokenStream) -> TokenStream {
    let out = using_param("&'static self, ctx: i32".parse().unwrap(), "
impl Foo {
    pub fn foo(&self, s: &str) -> &str {
        s
    }
    pub fn bar(&mut self) -> i32 {
        ctx
    }
    pub fn baz(self: &Self) -> i32 {
        ctx
    }
    pub fn a(this: &Self) -> i32 {
        ctx
    }
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    pub fn foo(& self, ctx : i32, s : & str) -> & str {
        s
    }
    pub fn bar(& mut self, ctx : i32) -> i32 {
        ctx
    }
    pub fn baz(self : & Self, ctx : i32) -> i32 {
        ctx
    }
    pub fn a(& 'static self, ctx : i32, this : & Self) -> i32 {
        ctx
    }
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}


/// ```
/// using_param::__test_self_param! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_self_param_after(_: TokenStream) -> TokenStream {
    let out = using_param(", &'static self, ctx: i32".parse().unwrap(), "
impl Foo {
    pub fn foo(&self, s: &str) -> &str {
        s
    }
    pub fn bar(&mut self) -> i32 {
        ctx
    }
    pub fn baz(self: &Self) -> i32 {
        ctx
    }
    pub fn a(this: &Self) -> i32 {
        ctx
    }
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    pub fn foo(& self, s : & str, ctx : i32) -> & str {
        s
    }
    pub fn bar(& mut self, ctx : i32) -> i32 {
        ctx
    }
    pub fn baz(self : & Self, ctx : i32) -> i32 {
        ctx
    }
    pub fn a(& 'static self, this : & Self, ctx : i32) -> i32 {
        ctx
    }
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}


/// ```
/// using_param::__test_generic_before! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_generic_before(_: TokenStream) -> TokenStream {
    let out = using_generic("'a".parse().unwrap(), "
impl Foo {
    fn foo() {}
    fn bar<'b>() {}
    fn baz<'b, T>() {}
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    fn foo < 'a > () {}
    fn bar < 'a, 'b > () {}
    fn baz < 'a, 'b, T > () {}
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}


/// ```
/// using_param::__test_generic_after! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_generic_after(_: TokenStream) -> TokenStream {
    let out = using_generic(", 'a".parse().unwrap(), "
impl Foo {
    fn foo() {}
    fn bar<'b>() {}
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    fn foo < 'a > () {}
    fn bar < 'b, 'a > () {}
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}


/// ```
/// using_param::__test_other_assoc_item! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_other_assoc_item(_: TokenStream) -> TokenStream {
    let out = using_param("ctx: i32".parse().unwrap(), "
impl Foo {
    pub const M: usize = 3;
    pub type C = i32;
    some_macro!();
    fn foo() {}
    fn bar(m: i32) { m+ctx }
    fn baz(self, m: i32) { m+ctx }
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    pub const M : usize = 3;
    pub type C = i32;
    some_macro! ();
    fn foo(ctx : i32) {}
    fn bar(ctx : i32, m : i32) { m+ctx }
    fn baz(self, ctx : i32, m : i32) { m+ctx }
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}


/// ```
/// using_param::__test_return_type! {}
/// ```
#[doc(hidden)]
#[proc_macro]
pub fn __test_return_type(_: TokenStream) -> TokenStream {
    let out = using_return("i32".parse().unwrap(), "
impl Foo {
    pub const M: usize = 3;
    pub type C = i32;
    some_macro!();
    fn foo() {}
    fn bar(m: i32) { m+ctx }
    fn baz(self, m: i32) { m+ctx }
    fn xfoo() -> u32 {}
    fn xbar(m: i32) -> u32 { m+ctx }
    fn xbaz(self, m: i32) -> u32 { m+ctx }
}
    ".parse().unwrap()).to_string();
    assert_eq!(out, "
impl Foo {
    pub const M : usize = 3;
    pub type C = i32;
    some_macro! ();
    fn foo() -> i32 {}
    fn bar(m : i32) -> i32 { m+ctx }
    fn baz(self, m : i32) -> i32 { m+ctx }
    fn xfoo() -> u32 {}
    fn xbar(m : i32) -> u32 { m+ctx }
    fn xbaz(self, m : i32) -> u32 { m+ctx }
}
    ".parse::<TokenStream>().unwrap().to_string());
    TokenStream::new()
}
