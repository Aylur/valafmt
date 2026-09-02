//! Hand-written recursive-descent parser producing the full-fidelity CST in
//! `syntax::ast`. Fails fast on the first syntax error (v1 refuses to format
//! files that don't parse, so multi-error recovery buys nothing yet).
//!
//! Disambiguation strategy (see plan): the token stream is fully lexed up
//! front, so speculation is a saved index + rollback. Used for:
//! - local declaration vs expression statement
//! - cast `(Type) x` vs parenthesized expression
//! - generic call `f<T> (x)` vs less-than
//! - lambda `(a, b) =>` vs parenthesized expression
//! - creation method `Foo ()` vs method with return type

use crate::lexer::token::{Keyword, Token, TokenKind};
use crate::syntax::ast::*;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

type PResult<T> = Result<T, ParseError>;

pub fn parse(tokens: Vec<Token>) -> Result<File, ParseError> {
    Parser { tokens, pos: 0 }.parse_file()
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

use Keyword as Kw;
use TokenKind as K;

/// Keywords that may appear where an identifier is expected (member names,
/// parameter names, etc.). valac requires `@` escaping for some of these, but
/// accepting them loosely costs a formatter nothing.
const CONTEXTUAL: &[Keyword] =
    &[Kw::Get, Kw::Set, Kw::Value, Kw::Default, Kw::Params, Kw::Global];

impl Parser {
    // -- token access -------------------------------------------------------

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn nth(&self, n: usize) -> &Token {
        &self.tokens[(self.pos + n).min(self.tokens.len() - 1)]
    }

    fn kind(&self) -> K {
        self.peek().kind
    }

    fn nth_kind(&self, n: usize) -> K {
        self.nth(n).kind
    }

    fn at(&self, kind: K) -> bool {
        self.kind() == kind
    }

    fn at_kw(&self, kw: Keyword) -> bool {
        self.kind() == K::Kw(kw)
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn error<T>(&self, message: impl Into<String>) -> PResult<T> {
        Err(ParseError { offset: self.peek().offset, message: message.into() })
    }

    fn expect(&mut self, kind: K, what: &str) -> PResult<Token> {
        if self.at(kind) {
            Ok(self.bump())
        } else {
            self.error(format!("expected {what}, found `{}`", self.describe()))
        }
    }

    fn expect_kw(&mut self, kw: Keyword, what: &str) -> PResult<Token> {
        self.expect(K::Kw(kw), what)
    }

    fn describe(&self) -> String {
        let t = self.peek();
        if t.kind == K::Eof {
            "end of file".to_string()
        } else {
            t.text.clone()
        }
    }

    fn eat(&mut self, kind: K) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            None
        }
    }

    /// Is the current token an identifier (including contextual keywords)?
    fn at_ident(&self) -> bool {
        match self.kind() {
            K::Ident => true,
            K::Kw(kw) => CONTEXTUAL.contains(&kw),
            _ => false,
        }
    }

    fn expect_ident(&mut self) -> PResult<Token> {
        if self.at_ident() {
            Ok(self.bump())
        } else {
            self.error(format!("expected identifier, found `{}`", self.describe()))
        }
    }

    /// Run `f`; on failure restore the position and return None.
    fn try_parse<T>(&mut self, f: impl FnOnce(&mut Self) -> PResult<T>) -> Option<T> {
        let save = self.pos;
        match f(self) {
            Ok(v) => Some(v),
            Err(_) => {
                self.pos = save;
                None
            }
        }
    }

    /// Are the current token and the next physically adjacent with no trivia
    /// between them? (Used to recombine `>` `>` into a shift.)
    fn next_adjacent(&self) -> bool {
        let a = self.peek();
        let b = self.nth(1);
        b.leading.is_empty() && a.trailing.is_empty() && b.offset == a.offset + a.text.len()
    }

    // -- file & members -----------------------------------------------------

    fn parse_file(&mut self) -> PResult<File> {
        let mut members = Vec::new();
        while !self.at(K::Eof) {
            members.push(self.parse_member()?);
        }
        let eof = self.bump();
        Ok(File { members, eof })
    }

    fn parse_member(&mut self) -> PResult<Member> {
        let mut attributes = Vec::new();
        while self.at(K::LBracket) {
            attributes.push(self.parse_attribute_list()?);
        }
        let kind = self.parse_member_kind()?;
        Ok(Member { attributes, kind })
    }

    fn at_modifier(&self) -> bool {
        matches!(
            self.kind(),
            K::Kw(Kw::Public)
                | K::Kw(Kw::Private)
                | K::Kw(Kw::Protected)
                | K::Kw(Kw::Internal)
                | K::Kw(Kw::Abstract)
                | K::Kw(Kw::Virtual)
                | K::Kw(Kw::Override)
                | K::Kw(Kw::Static)
                | K::Kw(Kw::Async)
                | K::Kw(Kw::Extern)
                | K::Kw(Kw::Inline)
                | K::Kw(Kw::Sealed)
                | K::Kw(Kw::Partial)
                | K::Kw(Kw::New)
                | K::Kw(Kw::Volatile)
        )
    }

    /// `class` acts as a member modifier (`class int count;`) unless it
    /// introduces a class declaration.
    fn class_is_decl(&self) -> bool {
        // `class` Name<...> { / : / . — a declaration; scan past the
        // qualified name and optional type parameters.
        let mut i = 1;
        if self.nth_kind(i) != K::Ident {
            return false;
        }
        i += 1;
        loop {
            match self.nth_kind(i) {
                K::Dot => {
                    if self.nth_kind(i + 1) == K::Ident {
                        i += 2;
                    } else {
                        return false;
                    }
                }
                K::Lt => {
                    // Skip <...> shallowly.
                    let mut depth = 1;
                    i += 1;
                    while depth > 0 {
                        match self.nth_kind(i) {
                            K::Lt => depth += 1,
                            K::Gt => depth -= 1,
                            K::Eof => return false,
                            _ => {}
                        }
                        i += 1;
                    }
                }
                K::LBrace | K::Colon => return true,
                _ => return false,
            }
        }
    }

    fn parse_modifiers(&mut self) -> Vec<Token> {
        let mut mods = Vec::new();
        loop {
            if self.at_modifier() {
                // `new` is a modifier only when a declaration follows; at
                // member level that's always the case.
                mods.push(self.bump());
            } else if self.at_kw(Kw::Class) && !self.class_is_decl() {
                mods.push(self.bump());
            } else {
                return mods;
            }
        }
    }

    fn parse_member_kind(&mut self) -> PResult<MemberKind> {
        if self.at_kw(Kw::Using) {
            return Ok(MemberKind::Using(self.parse_using()?));
        }
        if let Some(semi) = self.eat(K::Semicolon) {
            return Ok(MemberKind::Empty(semi));
        }
        let modifiers = self.parse_modifiers();
        match self.kind() {
            K::Kw(Kw::Namespace) => {
                let namespace_kw = self.bump();
                let name = self.parse_qualified_name()?;
                let body = self.parse_decl_body()?;
                Ok(MemberKind::Namespace(NamespaceDecl { modifiers, namespace_kw, name, body }))
            }
            K::Kw(Kw::Class) => Ok(MemberKind::Class(self.parse_type_decl(modifiers)?)),
            K::Kw(Kw::Interface) => Ok(MemberKind::Interface(self.parse_type_decl(modifiers)?)),
            K::Kw(Kw::Struct) => Ok(MemberKind::Struct(self.parse_type_decl(modifiers)?)),
            K::Kw(Kw::Enum) | K::Kw(Kw::Errordomain) => {
                Ok(self.parse_enum_decl(modifiers)?)
            }
            K::Kw(Kw::Delegate) => {
                let delegate_kw = self.bump();
                let return_type = self.parse_type()?;
                let name = self.parse_qualified_name()?;
                let type_params = self.parse_opt_type_params()?;
                let l_paren = self.expect(K::LParen, "`(`")?;
                let params = self.parse_params()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let throws = self.parse_opt_throws()?;
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(MemberKind::Delegate(DelegateDecl {
                    modifiers,
                    delegate_kw,
                    return_type,
                    name,
                    type_params,
                    l_paren,
                    params,
                    r_paren,
                    throws,
                    semicolon,
                }))
            }
            K::Kw(Kw::Signal) => {
                let signal_kw = self.bump();
                let return_type = self.parse_type()?;
                let name = self.expect_ident()?;
                let l_paren = self.expect(K::LParen, "`(`")?;
                let params = self.parse_params()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let body = self.parse_body()?;
                Ok(MemberKind::Signal(SignalDecl {
                    modifiers,
                    signal_kw,
                    return_type,
                    name,
                    l_paren,
                    params,
                    r_paren,
                    body,
                }))
            }
            K::Kw(Kw::Construct) => {
                let construct_kw = self.bump();
                let body = self.parse_block()?;
                Ok(MemberKind::Constructor(ConstructorDecl { modifiers, construct_kw, body }))
            }
            K::Tilde => {
                let tilde = self.bump();
                let name = self.expect_ident()?;
                let l_paren = self.expect(K::LParen, "`(`")?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let body = self.parse_block()?;
                Ok(MemberKind::Destructor(DestructorDecl {
                    modifiers,
                    tilde,
                    name,
                    l_paren,
                    r_paren,
                    body,
                }))
            }
            K::Kw(Kw::Const) => {
                let const_kw = self.bump();
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                let init = match self.eat(K::Assign) {
                    Some(eq) => Some((eq, self.parse_expr()?)),
                    None => None,
                };
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(MemberKind::Constant(ConstantDecl {
                    modifiers,
                    const_kw,
                    ty,
                    name,
                    init,
                    semicolon,
                }))
            }
            _ => self.parse_typed_member(modifiers),
        }
    }

    /// Members that start with a type (or a creation-method name): methods,
    /// creation methods, fields, properties.
    fn parse_typed_member(&mut self, modifiers: Vec<Token>) -> PResult<MemberKind> {
        // Creation method: QualifiedName directly followed by `(`.
        if let Some((name, l_paren)) = self.try_parse(|p| {
            let name = p.parse_qualified_name()?;
            let l_paren = p.expect(K::LParen, "`(`")?;
            Ok((name, l_paren))
        }) {
            let params = self.parse_params()?;
            let r_paren = self.expect(K::RParen, "`)`")?;
            let throws = self.parse_opt_throws()?;
            let contracts = self.parse_contracts()?;
            let body = self.parse_body()?;
            return Ok(MemberKind::CreationMethod(CreationMethodDecl {
                modifiers,
                name,
                l_paren,
                params,
                r_paren,
                throws,
                contracts,
                body,
            }));
        }

        let ty = self.parse_type()?;
        let name = self.parse_qualified_name()?;
        match self.kind() {
            K::LParen | K::Lt => {
                let type_params = self.parse_opt_type_params()?;
                let l_paren = self.expect(K::LParen, "`(`")?;
                let params = self.parse_params()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let throws = self.parse_opt_throws()?;
                let contracts = self.parse_contracts()?;
                let body = self.parse_body()?;
                Ok(MemberKind::Method(MethodDecl {
                    modifiers,
                    return_type: ty,
                    name,
                    type_params,
                    l_paren,
                    params,
                    r_paren,
                    throws,
                    contracts,
                    body,
                }))
            }
            K::LBrace => {
                let l_brace = self.bump();
                let mut accessors = Vec::new();
                while !self.at(K::RBrace) && !self.at(K::Eof) {
                    accessors.push(self.parse_property_accessor()?);
                }
                let r_brace = self.expect(K::RBrace, "`}`")?;
                Ok(MemberKind::Property(PropertyDecl {
                    modifiers,
                    ty,
                    name,
                    l_brace,
                    accessors,
                    r_brace,
                }))
            }
            _ => {
                if name.global.is_some() || name.parts.len() != 1 {
                    return self.error("expected `(`, `{`, `=`, or `;` after member name");
                }
                let name = name.parts.into_iter().next().unwrap();
                let init = match self.eat(K::Assign) {
                    Some(eq) => Some((eq, self.parse_expr()?)),
                    None => None,
                };
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(MemberKind::Field(FieldDecl { modifiers, ty, name, init, semicolon }))
            }
        }
    }

    fn parse_property_accessor(&mut self) -> PResult<PropertyAccessor> {
        let mut attributes = Vec::new();
        while self.at(K::LBracket) {
            attributes.push(self.parse_attribute_list()?);
        }
        if self.at_kw(Kw::Default) {
            let default_kw = self.bump();
            let eq = self.expect(K::Assign, "`=`")?;
            let expr = self.parse_expr()?;
            let semicolon = self.expect(K::Semicolon, "`;`")?;
            return Ok(PropertyAccessor::Default { default_kw, eq, expr, semicolon });
        }
        let mut modifiers = Vec::new();
        while matches!(
            self.kind(),
            K::Kw(Kw::Public)
                | K::Kw(Kw::Private)
                | K::Kw(Kw::Protected)
                | K::Kw(Kw::Internal)
                | K::Kw(Kw::Owned)
        ) {
            modifiers.push(self.bump());
        }
        let mut keywords = Vec::new();
        while matches!(self.kind(), K::Kw(Kw::Get) | K::Kw(Kw::Set) | K::Kw(Kw::Construct)) {
            keywords.push(self.bump());
        }
        if keywords.is_empty() {
            return self.error("expected `get`, `set`, `construct`, or `default` in property");
        }
        let body = self.parse_body()?;
        Ok(PropertyAccessor::Accessor { attributes, modifiers, keywords, body })
    }

    fn parse_using(&mut self) -> PResult<UsingDirective> {
        let using_kw = self.bump();
        let mut names = Punctuated::new();
        names.items.push(self.parse_qualified_name()?);
        while let Some(comma) = self.eat(K::Comma) {
            names.commas.push(comma);
            names.items.push(self.parse_qualified_name()?);
        }
        let semicolon = self.expect(K::Semicolon, "`;`")?;
        Ok(UsingDirective { using_kw, names, semicolon })
    }

    fn parse_type_decl(&mut self, modifiers: Vec<Token>) -> PResult<TypeDecl> {
        let keyword = self.bump();
        let name = self.parse_qualified_name()?;
        let type_params = self.parse_opt_type_params()?;
        let base = if self.at(K::Colon) {
            let colon = self.bump();
            let mut types = Punctuated::new();
            types.items.push(self.parse_type()?);
            while let Some(comma) = self.eat(K::Comma) {
                types.commas.push(comma);
                types.items.push(self.parse_type()?);
            }
            Some(BaseClause { colon, types })
        } else {
            None
        };
        let body = self.parse_decl_body()?;
        Ok(TypeDecl { modifiers, keyword, name, type_params, base, body })
    }

    fn parse_enum_decl(&mut self, modifiers: Vec<Token>) -> PResult<MemberKind> {
        let is_enum = self.at_kw(Kw::Enum);
        let keyword = self.bump();
        let name = self.parse_qualified_name()?;
        let base = if self.at(K::Colon) {
            let colon = self.bump();
            let mut types = Punctuated::new();
            types.items.push(self.parse_type()?);
            Some(BaseClause { colon, types })
        } else {
            None
        };
        let l_brace = self.expect(K::LBrace, "`{`")?;
        let mut values = Punctuated::new();
        loop {
            if self.at(K::RBrace) || self.at(K::Semicolon) || self.at(K::Eof) {
                break;
            }
            let mut attributes = Vec::new();
            while self.at(K::LBracket) {
                attributes.push(self.parse_attribute_list()?);
            }
            let value_name = self.expect_ident()?;
            let value = match self.eat(K::Assign) {
                Some(eq) => Some((eq, self.parse_expr()?)),
                None => None,
            };
            values.items.push(EnumValue { attributes, name: value_name, value });
            match self.eat(K::Comma) {
                Some(comma) => values.commas.push(comma),
                None => break,
            }
        }
        let semicolon = self.eat(K::Semicolon);
        let mut members = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            members.push(self.parse_member()?);
        }
        let r_brace = self.expect(K::RBrace, "`}`")?;
        let decl =
            EnumDecl { modifiers, keyword, name, base, l_brace, values, semicolon, members, r_brace };
        Ok(if is_enum { MemberKind::Enum(decl) } else { MemberKind::Errordomain(decl) })
    }

    fn parse_decl_body(&mut self) -> PResult<DeclBody> {
        let l_brace = self.expect(K::LBrace, "`{`")?;
        let mut members = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            members.push(self.parse_member()?);
        }
        let r_brace = self.expect(K::RBrace, "`}`")?;
        Ok(DeclBody { l_brace, members, r_brace })
    }

    fn parse_opt_type_params(&mut self) -> PResult<Option<TypeParamList>> {
        if !self.at(K::Lt) {
            return Ok(None);
        }
        let lt = self.bump();
        let mut params = Punctuated::new();
        params.items.push(self.expect_ident()?);
        while let Some(comma) = self.eat(K::Comma) {
            params.commas.push(comma);
            params.items.push(self.expect_ident()?);
        }
        let gt = self.expect(K::Gt, "`>`")?;
        Ok(Some(TypeParamList { lt, params, gt }))
    }

    fn parse_opt_throws(&mut self) -> PResult<Option<ThrowsClause>> {
        if !self.at_kw(Kw::Throws) {
            return Ok(None);
        }
        let throws_kw = self.bump();
        let mut types = Punctuated::new();
        types.items.push(self.parse_type()?);
        while let Some(comma) = self.eat(K::Comma) {
            types.commas.push(comma);
            types.items.push(self.parse_type()?);
        }
        Ok(Some(ThrowsClause { throws_kw, types }))
    }

    fn parse_contracts(&mut self) -> PResult<Vec<Contract>> {
        let mut out = Vec::new();
        while self.at_kw(Kw::Requires) || self.at_kw(Kw::Ensures) {
            let keyword = self.bump();
            let l_paren = self.expect(K::LParen, "`(`")?;
            let expr = self.parse_expr()?;
            let r_paren = self.expect(K::RParen, "`)`")?;
            out.push(Contract { keyword, l_paren, expr, r_paren });
        }
        Ok(out)
    }

    fn parse_body(&mut self) -> PResult<Body> {
        if let Some(semi) = self.eat(K::Semicolon) {
            Ok(Body::Semicolon(semi))
        } else {
            Ok(Body::Block(self.parse_block()?))
        }
    }

    fn parse_params(&mut self) -> PResult<Punctuated<Param>> {
        let mut params = Punctuated::new();
        if self.at(K::RParen) {
            return Ok(params);
        }
        loop {
            params.items.push(self.parse_param()?);
            match self.eat(K::Comma) {
                Some(comma) => params.commas.push(comma),
                None => break,
            }
        }
        Ok(params)
    }

    fn parse_param(&mut self) -> PResult<Param> {
        let mut attributes = Vec::new();
        while self.at(K::LBracket) {
            attributes.push(self.parse_attribute_list()?);
        }
        let mut modifiers = Vec::new();
        while matches!(
            self.kind(),
            K::Kw(Kw::Out) | K::Kw(Kw::Ref) | K::Kw(Kw::Owned) | K::Kw(Kw::Params)
        ) {
            // `params` is a modifier only before a type; `params` as a lone
            // identifier param name is unlikely, accept the simple reading.
            modifiers.push(self.bump());
        }
        if let Some(ellipsis) = self.eat(K::DotDotDot) {
            return Ok(Param { attributes, modifiers, kind: ParamKind::Ellipsis(ellipsis) });
        }
        let ty = self.parse_type()?;
        let name = self.expect_ident()?;
        let default = match self.eat(K::Assign) {
            Some(eq) => Some((eq, self.parse_expr()?)),
            None => None,
        };
        Ok(Param { attributes, modifiers, kind: ParamKind::Typed { ty, name, default } })
    }

    // -- attributes ---------------------------------------------------------

    fn parse_attribute_list(&mut self) -> PResult<AttributeList> {
        let l_bracket = self.bump();
        let mut attrs = Punctuated::new();
        loop {
            let name = self.expect_ident()?;
            let args = if self.at(K::LParen) {
                let l_paren = self.bump();
                let mut list = Punctuated::new();
                if !self.at(K::RParen) {
                    loop {
                        let arg_name = self.expect_ident()?;
                        let eq = self.expect(K::Assign, "`=`")?;
                        let value = self.parse_expr()?;
                        list.items.push(AttributeArg { name: arg_name, eq, value });
                        match self.eat(K::Comma) {
                            Some(comma) => list.commas.push(comma),
                            None => break,
                        }
                    }
                }
                let r_paren = self.expect(K::RParen, "`)`")?;
                Some(AttributeArgs { l_paren, args: list, r_paren })
            } else {
                None
            };
            attrs.items.push(Attribute { name, args });
            match self.eat(K::Comma) {
                Some(comma) => attrs.commas.push(comma),
                None => break,
            }
        }
        let r_bracket = self.expect(K::RBracket, "`]`")?;
        Ok(AttributeList { l_bracket, attrs, r_bracket })
    }

    // -- names & types ------------------------------------------------------

    fn parse_qualified_name(&mut self) -> PResult<QualifiedName> {
        let global = if self.at_kw(Kw::Global) && self.nth_kind(1) == K::ColonColon {
            let g = self.bump();
            let cc = self.bump();
            Some((g, cc))
        } else {
            None
        };
        let mut parts = vec![self.expect_ident()?];
        while self.at(K::Dot) && self.next_is_ident(1) {
            parts.push(self.bump()); // .
            parts.push(self.bump()); // ident
        }
        Ok(QualifiedName { global, parts })
    }

    fn next_is_ident(&self, n: usize) -> bool {
        match self.nth_kind(n) {
            K::Ident => true,
            K::Kw(kw) => CONTEXTUAL.contains(&kw),
            _ => false,
        }
    }

    fn at_type_prefix(&self) -> bool {
        matches!(
            self.kind(),
            K::Kw(Kw::Owned) | K::Kw(Kw::Unowned) | K::Kw(Kw::Weak) | K::Kw(Kw::Dynamic)
        )
    }

    fn parse_type(&mut self) -> PResult<Type> {
        let mut prefix = Vec::new();
        while self.at_type_prefix() {
            prefix.push(self.bump());
        }
        let core = if self.at_kw(Kw::Void) {
            TypeCore::Void(self.bump())
        } else {
            let name = self.parse_qualified_name()?;
            let type_args = if self.at(K::Lt) {
                match self.try_parse(|p| p.parse_type_arg_list()) {
                    Some(args) => Some(args),
                    None => None,
                }
            } else {
                None
            };
            TypeCore::Named { name, type_args }
        };
        let suffixes = self.parse_type_suffixes()?;
        Ok(Type { prefix, core, suffixes })
    }

    fn parse_type_suffixes(&mut self) -> PResult<Vec<TypeSuffix>> {
        let mut suffixes = Vec::new();
        loop {
            match self.kind() {
                K::Question => suffixes.push(TypeSuffix::Nullable(self.bump())),
                K::Star => suffixes.push(TypeSuffix::Pointer(self.bump())),
                K::LBracket => {
                    let l_bracket = self.bump();
                    let mut dims: Punctuated<Option<Expr>> = Punctuated::new();
                    if self.at(K::RBracket) {
                        dims.items.push(None);
                    } else {
                        loop {
                            if self.at(K::Comma) || self.at(K::RBracket) {
                                dims.items.push(None);
                            } else {
                                dims.items.push(Some(self.parse_expr()?));
                            }
                            match self.eat(K::Comma) {
                                Some(comma) => dims.commas.push(comma),
                                None => break,
                            }
                        }
                    }
                    let r_bracket = self.expect(K::RBracket, "`]`")?;
                    let nullable = self.eat(K::Question);
                    suffixes.push(TypeSuffix::Array { l_bracket, dims, r_bracket, nullable });
                }
                _ => break,
            }
        }
        Ok(suffixes)
    }

    fn parse_type_arg_list(&mut self) -> PResult<TypeArgList> {
        let lt = self.expect(K::Lt, "`<`")?;
        let mut args = Punctuated::new();
        args.items.push(self.parse_type()?);
        while let Some(comma) = self.eat(K::Comma) {
            args.commas.push(comma);
            args.items.push(self.parse_type()?);
        }
        let gt = self.expect(K::Gt, "`>`")?;
        Ok(TypeArgList { lt, args, gt })
    }

    // -- statements ---------------------------------------------------------

    pub fn parse_block(&mut self) -> PResult<Block> {
        let l_brace = self.expect(K::LBrace, "`{`")?;
        let mut statements = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            statements.push(self.parse_stmt()?);
        }
        let r_brace = self.expect(K::RBrace, "`}`")?;
        Ok(Block { l_brace, statements, r_brace })
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        match self.kind() {
            K::LBrace => Ok(Stmt::Block(self.parse_block()?)),
            K::Semicolon => Ok(Stmt::Empty(self.bump())),
            K::Kw(Kw::If) => self.parse_if(),
            K::Kw(Kw::Switch) => self.parse_switch(),
            K::Kw(Kw::While) => {
                let while_kw = self.bump();
                let l_paren = self.expect(K::LParen, "`(`")?;
                let cond = self.parse_expr()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let body = Box::new(self.parse_embedded_stmt()?);
                Ok(Stmt::While(WhileStmt { while_kw, l_paren, cond, r_paren, body }))
            }
            K::Kw(Kw::Do) => {
                let do_kw = self.bump();
                let body = Box::new(self.parse_embedded_stmt()?);
                let while_kw = self.expect_kw(Kw::While, "`while`")?;
                let l_paren = self.expect(K::LParen, "`(`")?;
                let cond = self.parse_expr()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(Stmt::DoWhile(DoWhileStmt {
                    do_kw,
                    body,
                    while_kw,
                    l_paren,
                    cond,
                    r_paren,
                    semicolon,
                }))
            }
            K::Kw(Kw::For) => self.parse_for(),
            K::Kw(Kw::Foreach) => self.parse_foreach(),
            K::Kw(Kw::Break) => {
                let kw = self.bump();
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(Stmt::Break { kw, semicolon })
            }
            K::Kw(Kw::Continue) => {
                let kw = self.bump();
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(Stmt::Continue { kw, semicolon })
            }
            K::Kw(Kw::Return) => {
                let kw = self.bump();
                let expr = if self.at(K::Semicolon) { None } else { Some(self.parse_expr()?) };
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(Stmt::Return { kw, expr, semicolon })
            }
            K::Kw(Kw::Throw) => {
                let kw = self.bump();
                let expr = self.parse_expr()?;
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(Stmt::Throw { kw, expr, semicolon })
            }
            K::Kw(Kw::Try) => self.parse_try(),
            K::Kw(Kw::Lock) => {
                let lock_kw = self.bump();
                let l_paren = self.expect(K::LParen, "`(`")?;
                let expr = self.parse_expr()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let body = self.parse_lock_body()?;
                Ok(Stmt::Lock(LockStmt { lock_kw, l_paren, expr, r_paren, body }))
            }
            K::Kw(Kw::Unlock) => {
                let kw = self.bump();
                let l_paren = self.expect(K::LParen, "`(`")?;
                let expr = self.parse_expr()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(Stmt::Unlock { kw, l_paren, expr, r_paren, semicolon })
            }
            K::Kw(Kw::Delete) => {
                let kw = self.bump();
                let expr = self.parse_expr()?;
                let semicolon = self.expect(K::Semicolon, "`;`")?;
                Ok(Stmt::Delete { kw, expr, semicolon })
            }
            K::Kw(Kw::Yield) if self.nth_kind(1) == K::Semicolon => {
                let kw = self.bump();
                let semicolon = self.bump();
                Ok(Stmt::Yield { kw, semicolon })
            }
            K::Kw(Kw::With) => self.parse_with(),
            _ => self.parse_decl_or_expr_stmt(),
        }
    }

    /// Body of if/while/for/etc. — any statement (usually a block).
    fn parse_embedded_stmt(&mut self) -> PResult<Stmt> {
        self.parse_stmt()
    }

    fn parse_lock_body(&mut self) -> PResult<Body> {
        if let Some(semi) = self.eat(K::Semicolon) {
            Ok(Body::Semicolon(semi))
        } else {
            Ok(Body::Block(self.parse_block()?))
        }
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        let if_kw = self.bump();
        let l_paren = self.expect(K::LParen, "`(`")?;
        let cond = self.parse_expr()?;
        let r_paren = self.expect(K::RParen, "`)`")?;
        let then = Box::new(self.parse_embedded_stmt()?);
        let else_branch = if self.at_kw(Kw::Else) {
            let else_kw = self.bump();
            let body = Box::new(self.parse_embedded_stmt()?);
            Some(ElseBranch { else_kw, body })
        } else {
            None
        };
        Ok(Stmt::If(IfStmt { if_kw, l_paren, cond, r_paren, then, else_branch }))
    }

    fn parse_switch(&mut self) -> PResult<Stmt> {
        let switch_kw = self.bump();
        let l_paren = self.expect(K::LParen, "`(`")?;
        let expr = self.parse_expr()?;
        let r_paren = self.expect(K::RParen, "`)`")?;
        let l_brace = self.expect(K::LBrace, "`{`")?;
        let mut sections = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            let mut labels = Vec::new();
            loop {
                if self.at_kw(Kw::Case) {
                    let case_kw = self.bump();
                    let case_expr = self.parse_expr()?;
                    let colon = self.expect(K::Colon, "`:`")?;
                    labels.push(SwitchLabel::Case { case_kw, expr: case_expr, colon });
                } else if self.at_kw(Kw::Default) && self.nth_kind(1) == K::Colon {
                    let default_kw = self.bump();
                    let colon = self.bump();
                    labels.push(SwitchLabel::Default { default_kw, colon });
                } else {
                    break;
                }
            }
            if labels.is_empty() {
                return self.error("expected `case` or `default` in switch body");
            }
            let mut statements = Vec::new();
            while !matches!(self.kind(), K::RBrace | K::Eof | K::Kw(Kw::Case))
                && !(self.at_kw(Kw::Default) && self.nth_kind(1) == K::Colon)
            {
                statements.push(self.parse_stmt()?);
            }
            sections.push(SwitchSection { labels, statements });
        }
        let r_brace = self.expect(K::RBrace, "`}`")?;
        Ok(Stmt::Switch(SwitchStmt { switch_kw, l_paren, expr, r_paren, l_brace, sections, r_brace }))
    }

    fn parse_for(&mut self) -> PResult<Stmt> {
        let for_kw = self.bump();
        let l_paren = self.expect(K::LParen, "`(`")?;
        let init = if self.at(K::Semicolon) {
            let semicolon = self.bump();
            ForInit::Exprs { exprs: Punctuated::new(), semicolon }
        } else if let Some(decl) = self.try_parse(|p| p.parse_local_decl()) {
            ForInit::Decl(decl)
        } else {
            let mut exprs = Punctuated::new();
            exprs.items.push(self.parse_expr()?);
            while let Some(comma) = self.eat(K::Comma) {
                exprs.commas.push(comma);
                exprs.items.push(self.parse_expr()?);
            }
            let semicolon = self.expect(K::Semicolon, "`;`")?;
            ForInit::Exprs { exprs, semicolon }
        };
        let cond = if self.at(K::Semicolon) { None } else { Some(self.parse_expr()?) };
        let semicolon2 = self.expect(K::Semicolon, "`;`")?;
        let mut update = Punctuated::new();
        if !self.at(K::RParen) {
            update.items.push(self.parse_expr()?);
            while let Some(comma) = self.eat(K::Comma) {
                update.commas.push(comma);
                update.items.push(self.parse_expr()?);
            }
        }
        let r_paren = self.expect(K::RParen, "`)`")?;
        let body = Box::new(self.parse_embedded_stmt()?);
        Ok(Stmt::For(ForStmt { for_kw, l_paren, init, cond, semicolon2, update, r_paren, body }))
    }

    fn parse_local_decl_type(&mut self) -> PResult<LocalDeclType> {
        let mut prefix = Vec::new();
        while self.at_type_prefix() {
            prefix.push(self.bump());
        }
        if self.at_kw(Kw::Var) {
            let var_kw = self.bump();
            let nullable = self.eat(K::Question);
            Ok(LocalDeclType::Var { prefix, var_kw, nullable })
        } else {
            // Re-parse prefix as part of the full type.
            let core = if self.at_kw(Kw::Void) {
                TypeCore::Void(self.bump())
            } else {
                let name = self.parse_qualified_name()?;
                let type_args = if self.at(K::Lt) {
                    self.try_parse(|p| p.parse_type_arg_list())
                } else {
                    None
                };
                TypeCore::Named { name, type_args }
            };
            let suffixes = self.parse_type_suffixes()?;
            Ok(LocalDeclType::Type(Type { prefix, core, suffixes }))
        }
    }

    fn parse_foreach(&mut self) -> PResult<Stmt> {
        let foreach_kw = self.bump();
        let l_paren = self.expect(K::LParen, "`(`")?;
        let ty = self.parse_local_decl_type()?;
        let name = self.expect_ident()?;
        let in_kw = self.expect_kw(Kw::In, "`in`")?;
        let expr = self.parse_expr()?;
        let r_paren = self.expect(K::RParen, "`)`")?;
        let body = Box::new(self.parse_embedded_stmt()?);
        Ok(Stmt::Foreach(ForeachStmt { foreach_kw, l_paren, ty, name, in_kw, expr, r_paren, body }))
    }

    fn parse_try(&mut self) -> PResult<Stmt> {
        let try_kw = self.bump();
        let body = self.parse_block()?;
        let mut catches = Vec::new();
        while self.at_kw(Kw::Catch) {
            let catch_kw = self.bump();
            let param = if self.at(K::LParen) {
                let l_paren = self.bump();
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                Some(CatchParam { l_paren, ty, name, r_paren })
            } else {
                None
            };
            let catch_body = self.parse_block()?;
            catches.push(CatchClause { catch_kw, param, body: catch_body });
        }
        let finally = if self.at_kw(Kw::Finally) {
            let finally_kw = self.bump();
            let fin_body = self.parse_block()?;
            Some(FinallyClause { finally_kw, body: fin_body })
        } else {
            None
        };
        Ok(Stmt::Try(TryStmt { try_kw, body, catches, finally }))
    }

    fn parse_with(&mut self) -> PResult<Stmt> {
        let with_kw = self.bump();
        let l_paren = self.expect(K::LParen, "`(`")?;
        // `with (var x = expr)` or `with (expr)`.
        let decl = self.try_parse(|p| {
            let ty = p.parse_local_decl_type()?;
            let name = p.expect_ident()?;
            let eq = p.expect(K::Assign, "`=`")?;
            Ok((ty, name, eq))
        });
        let expr = self.parse_expr()?;
        let r_paren = self.expect(K::RParen, "`)`")?;
        let body = self.parse_block()?;
        Ok(Stmt::With(WithStmt { with_kw, l_paren, decl, expr, r_paren, body }))
    }

    fn parse_decl_or_expr_stmt(&mut self) -> PResult<Stmt> {
        if let Some(decl) = self.try_parse(|p| p.parse_local_decl()) {
            return Ok(Stmt::LocalDecl(decl));
        }
        let expr = self.parse_expr()?;
        let semicolon = self.expect(K::Semicolon, "`;`")?;
        Ok(Stmt::Expr { expr, semicolon })
    }

    /// `[const] Type name [= expr][, name [= expr]]* ;`
    fn parse_local_decl(&mut self) -> PResult<LocalDecl> {
        let const_kw = self.eat(K::Kw(Kw::Const));
        let ty = self.parse_local_decl_type()?;
        // Commit test: a declaration has an identifier here followed by
        // `=`, `;`, or `,`.
        if !self.at_ident() {
            return self.error("not a declaration");
        }
        if !matches!(self.nth_kind(1), K::Assign | K::Semicolon | K::Comma) {
            return self.error("not a declaration");
        }
        let mut declarators = Punctuated::new();
        loop {
            let name = self.expect_ident()?;
            let init = match self.eat(K::Assign) {
                Some(eq) => Some((eq, self.parse_expr()?)),
                None => None,
            };
            declarators.items.push(Declarator { name, init });
            match self.eat(K::Comma) {
                Some(comma) => declarators.commas.push(comma),
                None => break,
            }
        }
        let semicolon = self.expect(K::Semicolon, "`;`")?;
        Ok(LocalDecl { const_kw, ty, declarators, semicolon })
    }

    // -- expressions --------------------------------------------------------

    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> PResult<Expr> {
        if let Some(lambda) = self.try_parse_lambda()? {
            return Ok(lambda);
        }
        let lhs = self.parse_conditional()?;
        let op: Vec<Token> = match self.kind() {
            K::Assign
            | K::PlusEq
            | K::MinusEq
            | K::StarEq
            | K::SlashEq
            | K::PercentEq
            | K::AmpEq
            | K::PipeEq
            | K::CaretEq
            | K::ShlEq => vec![self.bump()],
            // `>>=` is `>` `>=` adjacent.
            K::Gt if self.nth_kind(1) == K::GtEq && self.next_adjacent() => {
                vec![self.bump(), self.bump()]
            }
            _ => return Ok(lhs),
        };
        let rhs = self.parse_assignment()?;
        Ok(Expr::Assign { lhs: Box::new(lhs), op, rhs: Box::new(rhs) })
    }

    /// Detect and parse a lambda: `ident =>` or `(ident, ...) =>`.
    fn try_parse_lambda(&mut self) -> PResult<Option<Expr>> {
        let is_lambda = if self.at_ident() && self.nth_kind(1) == K::FatArrow {
            true
        } else if self.at(K::LParen) {
            // Scan: ( [ident [, ident]*] ) =>
            let mut i = 1;
            loop {
                match self.nth_kind(i) {
                    K::RParen => break,
                    K::Ident | K::Kw(_) => {
                        i += 1;
                        match self.nth_kind(i) {
                            K::Comma => i += 1,
                            K::RParen => break,
                            _ => return Ok(None),
                        }
                    }
                    _ => return Ok(None),
                }
            }
            self.nth_kind(i + 1) == K::FatArrow
        } else {
            false
        };
        if !is_lambda {
            return Ok(None);
        }
        let params = if self.at(K::LParen) {
            let l_paren = self.bump();
            let mut params = Punctuated::new();
            if !self.at(K::RParen) {
                loop {
                    params.items.push(self.bump());
                    match self.eat(K::Comma) {
                        Some(comma) => params.commas.push(comma),
                        None => break,
                    }
                }
            }
            let r_paren = self.expect(K::RParen, "`)`")?;
            LambdaParams::List { l_paren, params, r_paren }
        } else {
            LambdaParams::Single(self.bump())
        };
        let arrow = self.expect(K::FatArrow, "`=>`")?;
        let body = if self.at(K::LBrace) {
            LambdaBody::Block(self.parse_block()?)
        } else {
            LambdaBody::Expr(Box::new(self.parse_expr()?))
        };
        Ok(Some(Expr::Lambda { params, arrow, body }))
    }

    fn parse_conditional(&mut self) -> PResult<Expr> {
        let cond = self.parse_coalescing()?;
        if !self.at(K::Question) {
            return Ok(cond);
        }
        let question = self.bump();
        let then = self.parse_expr_no_stmt_keywords()?;
        let colon = self.expect(K::Colon, "`:`")?;
        let els = self.parse_conditional_branch()?;
        Ok(Expr::Ternary {
            cond: Box::new(cond),
            question,
            then: Box::new(then),
            colon,
            els: Box::new(els),
        })
    }

    fn parse_expr_no_stmt_keywords(&mut self) -> PResult<Expr> {
        // The then-branch of `?:` must not consume the `:`; coalescing level
        // and below never do.
        if let Some(lambda) = self.try_parse_lambda()? {
            return Ok(lambda);
        }
        self.parse_coalescing()
    }

    fn parse_conditional_branch(&mut self) -> PResult<Expr> {
        // Right-associative nesting: `a ? b : c ? d : e`.
        self.parse_conditional()
    }

    fn parse_coalescing(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_or()?;
        while self.at(K::QuestionQuestion) {
            let op = vec![self.bump()];
            let rhs = self.parse_or()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_or(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.at(K::PipePipe) {
            let op = vec![self.bump()];
            let rhs = self.parse_and()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_bit_or()?;
        while self.at(K::AmpAmp) {
            let op = vec![self.bump()];
            let rhs = self.parse_bit_or()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_bit_or(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_bit_xor()?;
        while self.at(K::Pipe) {
            let op = vec![self.bump()];
            let rhs = self.parse_bit_xor()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_bit_xor(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_bit_and()?;
        while self.at(K::Caret) {
            let op = vec![self.bump()];
            let rhs = self.parse_bit_and()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_bit_and(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_equality()?;
        while self.at(K::Amp) {
            let op = vec![self.bump()];
            let rhs = self.parse_equality()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_relational()?;
        loop {
            match self.kind() {
                K::Eq | K::NotEq | K::Kw(Kw::In) => {
                    let op = vec![self.bump()];
                    let rhs = self.parse_relational()?;
                    lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn parse_relational(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_shift()?;
        loop {
            match self.kind() {
                K::Lt | K::LtEq | K::GtEq => {
                    let op = vec![self.bump()];
                    let rhs = self.parse_shift()?;
                    lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
                }
                // Lone `>` is relational; `>` `>` adjacent is a shift
                // (handled one level down) and `>` `>=` adjacent is `>>=`
                // (handled by parse_assignment).
                K::Gt if !(matches!(self.nth_kind(1), K::Gt | K::GtEq) && self.next_adjacent()) => {
                    let op = vec![self.bump()];
                    let rhs = self.parse_shift()?;
                    lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
                }
                K::Kw(Kw::Is) | K::Kw(Kw::As) => {
                    let op = self.bump();
                    let ty = self.parse_type()?;
                    lhs = Expr::TypeOp { expr: Box::new(lhs), op, ty };
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn parse_shift(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_additive()?;
        loop {
            match self.kind() {
                K::Shl => {
                    let op = vec![self.bump()];
                    let rhs = self.parse_additive()?;
                    lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
                }
                K::Gt if self.nth_kind(1) == K::Gt
                    && self.next_adjacent()
                    && self.nth_kind(2) != K::Assign =>
                {
                    let op = vec![self.bump(), self.bump()];
                    let rhs = self.parse_additive()?;
                    lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn parse_additive(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_multiplicative()?;
        while matches!(self.kind(), K::Plus | K::Minus) {
            let op = vec![self.bump()];
            let rhs = self.parse_multiplicative()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> PResult<Expr> {
        let mut lhs = self.parse_unary()?;
        while matches!(self.kind(), K::Star | K::Slash | K::Percent) {
            let op = vec![self.bump()];
            let rhs = self.parse_unary()?;
            lhs = Expr::Binary { lhs: Box::new(lhs), op, rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        match self.kind() {
            K::Bang | K::Tilde | K::Plus | K::Minus | K::PlusPlus | K::MinusMinus | K::Star
            | K::Amp => {
                let op = self.bump();
                let expr = self.parse_unary()?;
                Ok(Expr::Unary { op, expr: Box::new(expr) })
            }
            K::Kw(Kw::Yield) => {
                let yield_kw = self.bump();
                let expr = self.parse_unary()?;
                Ok(Expr::Yield { yield_kw, expr: Box::new(expr) })
            }
            K::LParen => {
                // Try `(owned) expr`, `(!) expr`, `(Type) expr`.
                if self.nth_kind(1) == K::Kw(Kw::Owned) && self.nth_kind(2) == K::RParen {
                    let l_paren = self.bump();
                    let owned_kw = self.bump();
                    let r_paren = self.bump();
                    let expr = self.parse_unary()?;
                    return Ok(Expr::OwnershipTransfer {
                        l_paren,
                        owned_kw,
                        r_paren,
                        expr: Box::new(expr),
                    });
                }
                if self.nth_kind(1) == K::Bang && self.nth_kind(2) == K::RParen {
                    let l_paren = self.bump();
                    let bang = self.bump();
                    let r_paren = self.bump();
                    let expr = self.parse_unary()?;
                    return Ok(Expr::NonNullCast { l_paren, bang, r_paren, expr: Box::new(expr) });
                }
                if let Some(cast) = self.try_parse(|p| p.parse_cast()) {
                    return Ok(cast);
                }
                self.parse_postfix()
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_cast(&mut self) -> PResult<Expr> {
        let l_paren = self.expect(K::LParen, "`(`")?;
        let ty = self.parse_type()?;
        let r_paren = self.expect(K::RParen, "`)`")?;
        // Only commit when the next token can begin a unary expression and is
        // not an operator that would also read as a binary continuation
        // (protects `(a) - b`, `(a) * b`, `(a) & b`).
        let commit = match self.kind() {
            K::Ident
            | K::IntLit
            | K::RealLit
            | K::CharLit
            | K::StringLit
            | K::VerbatimStringLit
            | K::TemplateStringLit
            | K::RegexLit
            | K::LParen
            | K::Bang
            | K::Tilde
            | K::Kw(Kw::This)
            | K::Kw(Kw::Base)
            | K::Kw(Kw::New)
            | K::Kw(Kw::True)
            | K::Kw(Kw::False)
            | K::Kw(Kw::Null)
            | K::Kw(Kw::Typeof)
            | K::Kw(Kw::Sizeof)
            | K::Kw(Kw::Yield)
            | K::Kw(Kw::Global) => true,
            K::Kw(kw) => CONTEXTUAL.contains(&kw),
            _ => false,
        };
        if !commit {
            return self.error("not a cast");
        }
        let expr = self.parse_unary()?;
        Ok(Expr::Cast { l_paren, ty, r_paren, expr: Box::new(expr) })
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.kind() {
                K::Dot | K::Arrow => {
                    let op = self.bump();
                    // After `.` any keyword reads unambiguously as a member
                    // name (`list.foreach`, `object.get`, ...).
                    let name = if matches!(self.kind(), K::Ident | K::Kw(_)) {
                        self.bump()
                    } else {
                        return self.error(format!(
                            "expected member name, found `{}`",
                            self.describe()
                        ));
                    };
                    // Generic member reference: `foo.bar<int> (...)`.
                    let type_args = if self.at(K::Lt) {
                        self.try_parse(|p| {
                            let args = p.parse_type_arg_list()?;
                            if p.at(K::LParen) {
                                Ok(args)
                            } else {
                                p.error("not a generic invocation")
                            }
                        })
                    } else {
                        None
                    };
                    expr = Expr::Member { base: Box::new(expr), op, name, type_args };
                }
                K::LParen => {
                    let l_paren = self.bump();
                    let args = self.parse_args()?;
                    let r_paren = self.expect(K::RParen, "`)`")?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        type_args: None,
                        l_paren,
                        args,
                        r_paren,
                    };
                }
                // Generic invocation on a bare callee: `f<int> (...)`.
                K::Lt => {
                    let type_args = self.try_parse(|p| {
                        let args = p.parse_type_arg_list()?;
                        if p.at(K::LParen) {
                            Ok(args)
                        } else {
                            p.error("not a generic invocation")
                        }
                    });
                    match type_args {
                        Some(type_args) => {
                            let l_paren = self.bump();
                            let args = self.parse_args()?;
                            let r_paren = self.expect(K::RParen, "`)`")?;
                            expr = Expr::Call {
                                callee: Box::new(expr),
                                type_args: Some(type_args),
                                l_paren,
                                args,
                                r_paren,
                            };
                        }
                        None => return Ok(expr),
                    }
                }
                K::LBracket => {
                    let l_bracket = self.bump();
                    // Slice `[a:b]` or index list `[a, b]`.
                    let first = if self.at(K::Colon) { None } else { Some(self.parse_expr()?) };
                    if self.at(K::Colon) {
                        let colon = self.bump();
                        let end = if self.at(K::RBracket) {
                            None
                        } else {
                            Some(Box::new(self.parse_expr()?))
                        };
                        let r_bracket = self.expect(K::RBracket, "`]`")?;
                        expr = Expr::Slice {
                            base: Box::new(expr),
                            l_bracket,
                            start: first.map(Box::new),
                            colon,
                            end,
                            r_bracket,
                        };
                    } else {
                        let mut indices = Punctuated::new();
                        if let Some(f) = first {
                            indices.items.push(f);
                            while let Some(comma) = self.eat(K::Comma) {
                                indices.commas.push(comma);
                                indices.items.push(self.parse_expr()?);
                            }
                        }
                        let r_bracket = self.expect(K::RBracket, "`]`")?;
                        expr = Expr::Index { base: Box::new(expr), l_bracket, indices, r_bracket };
                    }
                }
                K::PlusPlus | K::MinusMinus => {
                    let op = self.bump();
                    expr = Expr::PostfixIncDec { base: Box::new(expr), op };
                }
                _ => return Ok(expr),
            }
        }
    }

    fn parse_args(&mut self) -> PResult<Punctuated<Arg>> {
        let mut args = Punctuated::new();
        if self.at(K::RParen) {
            return Ok(args);
        }
        loop {
            let modifier = match self.kind() {
                K::Kw(Kw::Out) | K::Kw(Kw::Ref) => Some(self.bump()),
                _ => None,
            };
            let name = if self.at_ident() && self.nth_kind(1) == K::Colon {
                let n = self.bump();
                let colon = self.bump();
                Some((n, colon))
            } else {
                None
            };
            let expr = self.parse_expr()?;
            args.items.push(Arg { modifier, name, expr });
            match self.eat(K::Comma) {
                Some(comma) => args.commas.push(comma),
                None => break,
            }
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        match self.kind() {
            K::Ident
            | K::IntLit
            | K::RealLit
            | K::CharLit
            | K::StringLit
            | K::VerbatimStringLit
            | K::TemplateStringLit
            | K::RegexLit
            | K::Kw(Kw::This)
            | K::Kw(Kw::Base)
            | K::Kw(Kw::True)
            | K::Kw(Kw::False)
            | K::Kw(Kw::Null) => Ok(Expr::Atom(self.bump())),
            K::Kw(Kw::Global) if self.nth_kind(1) == K::ColonColon => {
                let global_kw = self.bump();
                let colon_colon = self.bump();
                let name = self.expect_ident()?;
                Ok(Expr::Global { global_kw, colon_colon, name })
            }
            K::Kw(kw) if CONTEXTUAL.contains(&kw) => Ok(Expr::Atom(self.bump())),
            K::LParen => {
                let l_paren = self.bump();
                let expr = self.parse_expr()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                Ok(Expr::Paren { l_paren, expr: Box::new(expr), r_paren })
            }
            K::LBrace => {
                Ok(Expr::Initializer(self.parse_initializer_list()?))
            }
            K::Kw(Kw::New) => {
                let new_kw = self.bump();
                let ty = self.parse_type()?;
                let args = if self.at(K::LParen) {
                    let l_paren = self.bump();
                    let list = self.parse_args()?;
                    let r_paren = self.expect(K::RParen, "`)`")?;
                    Some(CallArgs { l_paren, args: list, r_paren })
                } else {
                    None
                };
                let initializer = if self.at(K::LBrace) {
                    Some(self.parse_initializer_list()?)
                } else {
                    None
                };
                Ok(Expr::New { new_kw, ty, args, initializer })
            }
            K::Kw(Kw::Typeof) => {
                let kw = self.bump();
                let l_paren = self.expect(K::LParen, "`(`")?;
                let ty = self.parse_type()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                Ok(Expr::Typeof { kw, l_paren, ty, r_paren })
            }
            K::Kw(Kw::Sizeof) => {
                let kw = self.bump();
                let l_paren = self.expect(K::LParen, "`(`")?;
                let ty = self.parse_type()?;
                let r_paren = self.expect(K::RParen, "`)`")?;
                Ok(Expr::Sizeof { kw, l_paren, ty, r_paren })
            }
            _ => self.error(format!("expected expression, found `{}`", self.describe())),
        }
    }

    fn parse_initializer_list(&mut self) -> PResult<InitializerList> {
        let l_brace = self.expect(K::LBrace, "`{`")?;
        let mut items = Punctuated::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            items.items.push(self.parse_expr()?);
            match self.eat(K::Comma) {
                Some(comma) => items.commas.push(comma),
                None => break,
            }
        }
        let r_brace = self.expect(K::RBrace, "`}`")?;
        Ok(InitializerList { l_brace, items, r_brace })
    }
}
