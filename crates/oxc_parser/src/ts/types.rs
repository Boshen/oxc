use bitflags::bitflags;
use oxc_allocator::{Box, Vec};
use oxc_ast::{ast::*, context::Context};
use oxc_diagnostics::Result;

use super::list::{
    TSInterfaceOrObjectBodyList, TSTupleElementList, TSTypeArgumentList, TSTypeParameterList,
};
use crate::{
    diagnostics,
    js::list::{ArrayPatternList, ObjectPatternProperties},
    lexer::Kind,
    list::{NormalList, SeparatedList},
    Parser,
};

bitflags! {
  /// Bitflag of modifiers and contextual modifiers.
  /// Useful to cheaply track all already seen modifiers of a member (instead of using a HashSet<ModifierKind>).
  #[derive(Default)]
  pub struct ModifierFlags: u16 {
      const DECLARE       = 1 << 0;
      const PRIVATE       = 1 << 1;
      const PROTECTED     = 1 << 2;
      const PUBLIC        = 1 << 3;
      const STATIC        = 1 << 4;
      const READONLY      = 1 << 5;
      const ABSTRACT      = 1 << 6;
      const OVERRIDE      = 1 << 7;
      const ASYNC         = 1 << 8;
      const CONST         = 1 << 9;
      const IN            = 1 << 10;
      const OUT           = 1 << 11;
      const EXPORT        = 1 << 12;
      const DEFAULT       = 1 << 13;
      const ACCESSOR      = 1 << 14;
      const ACCESSIBILITY = Self::PRIVATE.bits | Self::PROTECTED.bits | Self::PUBLIC.bits;
  }
}

/// It is the caller's savety to always check by `Kind::is_modifier_kind`
/// before converting [`Kind`] to [`ModifierFlags`] so that we can assume here that
/// the convertion always succeeds.
impl From<Kind> for ModifierFlags {
    fn from(value: Kind) -> Self {
        match value {
            Kind::Abstract => Self::ABSTRACT,
            Kind::Declare => Self::DECLARE,
            Kind::Private => Self::PRIVATE,
            Kind::Protected => Self::PROTECTED,
            Kind::Public => Self::PUBLIC,
            Kind::Static => Self::STATIC,
            Kind::Readonly => Self::READONLY,
            Kind::Override => Self::OVERRIDE,
            Kind::Async => Self::ASYNC,
            Kind::Const => Self::CONST,
            Kind::In => Self::IN,
            Kind::Out => Self::OUT,
            Kind::Export => Self::EXPORT,
            Kind::Default => Self::DEFAULT,
            Kind::Accessor => Self::ACCESSOR,
            _ => unreachable!(),
        }
    }
}

impl ModifierFlags {
    pub fn accessibility(self) -> Option<TSAccessibility> {
        if self.contains(Self::PUBLIC) {
            return Some(TSAccessibility::Public);
        }
        if self.contains(Self::PROTECTED) {
            return Some(TSAccessibility::Protected);
        }

        if self.contains(Self::PRIVATE) {
            return Some(TSAccessibility::Private);
        }
        None
    }

    pub fn readonly(self) -> bool {
        self.contains(Self::READONLY)
    }

    pub fn declare(self) -> bool {
        self.contains(Self::DECLARE)
    }

    pub fn r#async(self) -> bool {
        self.contains(Self::ASYNC)
    }

    pub fn r#override(self) -> bool {
        self.contains(Self::OVERRIDE)
    }

    pub fn r#abstract(self) -> bool {
        self.contains(Self::ABSTRACT)
    }

    pub fn r#static(self) -> bool {
        self.contains(Self::STATIC)
    }
}

impl<'a> Parser<'a> {
    pub fn parse_ts_type(&mut self) -> Result<TSType<'a>> {
        if self.is_at_constructor_type() {
            return self.parse_ts_constructor_type();
        }

        if self.is_at_function_type() {
            return self.parse_ts_function_type();
        }

        let left = self.parse_ts_union_type()?;

        self.parse_ts_conditional_type(left)
    }

    pub fn parse_ts_type_parameters(
        &mut self,
    ) -> Result<Option<Box<'a, TSTypeParameterDeclaration<'a>>>> {
        if !self.ts_enabled() {
            return Ok(None);
        }
        if !self.at(Kind::LAngle) {
            return Ok(None);
        }
        let span = self.start_span();
        let params = TSTypeParameterList::parse(self)?.params;
        Ok(Some(self.ast.ts_type_parameters(self.end_span(span), params)))
    }

    pub fn parse_ts_implements_clause(
        &mut self,
    ) -> Result<Vec<'a, Box<'a, TSClassImplements<'a>>>> {
        self.expect(Kind::Implements)?;
        let first = self.parse_ts_implement_name()?;
        let mut implements = self.ast.new_vec();
        implements.push(first);

        while self.eat(Kind::Comma) {
            implements.push(self.parse_ts_implement_name()?);
        }

        Ok(implements)
    }

    pub fn parse_ts_type_parameter(&mut self) -> Result<Box<'a, TSTypeParameter<'a>>> {
        let span = self.start_span();

        let r#in = if self.at(Kind::In) && self.peek_kind().is_identifier_name() {
            self.bump_any();
            true
        } else {
            false
        };
        let out = if self.at(Kind::Out) && self.peek_kind().is_identifier_name() {
            self.bump_any();
            true
        } else {
            false
        };

        if self.at(Kind::In) && self.peek_kind().is_identifier_name() {
            // TODO error
        }

        let name = self.parse_binding_identifier()?;
        let constraint = self.parse_ts_type_constraint()?;
        let default = self.parse_ts_default_type()?;

        Ok(self.ast.ts_type_parameter(self.end_span(span), name, constraint, default, r#in, out))
    }

    fn parse_ts_type_constraint(&mut self) -> Result<Option<TSType<'a>>> {
        if !self.at(Kind::Extends) {
            return Ok(None);
        }
        self.bump_any();
        Ok(Some(self.parse_ts_type()?))
    }

    fn parse_ts_default_type(&mut self) -> Result<Option<TSType<'a>>> {
        if !self.at(Kind::Eq) {
            return Ok(None);
        }
        self.bump_any();
        Ok(Some(self.parse_ts_type()?))
    }

    fn parse_ts_conditional_type(&mut self, left: TSType<'a>) -> Result<TSType<'a>> {
        let span = self.start_span();
        if !self.ctx.has_disallow_conditional_types()
            && !self.cur_token().is_on_new_line
            && self.eat(Kind::Extends)
        {
            let extends_type =
                self.with_context(Context::DisallowConditionalTypes, Self::parse_ts_type)?;

            self.expect(Kind::Question)?;

            let true_type =
                self.without_context(Context::DisallowConditionalTypes, Self::parse_ts_type)?;

            self.expect(Kind::Colon)?;

            let false_type =
                self.without_context(Context::DisallowConditionalTypes, Self::parse_ts_type)?;

            return Ok(self.ast.ts_conditional_type(
                self.end_span(span),
                left,
                extends_type,
                true_type,
                false_type,
            ));
        }

        Ok(left)
    }

    fn is_at_constructor_type(&mut self) -> bool {
        self.at(Kind::New) || (self.at(Kind::Abstract) && self.peek_at(Kind::New))
    }

    // test ts ts_union_type
    // type A = string | number;
    // type B = | A | void | null;
    // type C = A & C | C;
    fn parse_ts_union_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        if self.at(Kind::Pipe) {
            let mut types = self.ast.new_vec();
            while self.eat(Kind::Pipe) {
                types.push(self.parse_ts_intersection_type()?);
            }
            Ok(self.ast.ts_union_type(self.end_span(span), types))
        } else {
            let first = self.parse_ts_intersection_type()?;
            if self.at(Kind::Pipe) {
                let mut types = self.ast.new_vec();
                types.push(first);
                while self.eat(Kind::Pipe) {
                    types.push(self.parse_ts_intersection_type()?);
                }
                Ok(self.ast.ts_union_type(self.end_span(span), types))
            } else {
                Ok(first)
            }
        }
    }

    // test ts ts_intersection_type
    // type A = string & number;
    // type B = & A & void & null;
    fn parse_ts_intersection_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        if self.at(Kind::Amp) {
            let mut types = self.ast.new_vec();
            while self.eat(Kind::Amp) {
                types.push(self.parse_ts_primary_type()?);
            }
            Ok(self.ast.ts_intersection_type(self.end_span(span), types))
        } else {
            let first = self.parse_ts_primary_type()?;
            if self.at(Kind::Amp) {
                let mut types = self.ast.new_vec();
                types.push(first);
                while self.eat(Kind::Amp) {
                    types.push(self.parse_ts_primary_type()?);
                }
                Ok(self.ast.ts_intersection_type(self.end_span(span), types))
            } else {
                Ok(first)
            }
        }
    }

    fn parse_ts_primary_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        if self.at(Kind::Infer) {
            return self.parse_ts_infer_type();
        }

        let mut operator = None;

        if !self.at(Kind::Str) {
            if let Some(atom) = self.cur_atom() {
                operator = TSTypeOperator::from_src(atom);
            }
        }

        // test ts ts_type_operator
        // type B = keyof A;
        // type C = readonly string[];
        // const d: unique symbol = Symbol();
        if let Some(operator) = operator {
            self.bump_any(); // bump operator
            let type_annotation = self.parse_ts_primary_type()?;
            return Ok(self.ast.ts_type_operator_type(
                self.end_span(span),
                operator,
                type_annotation,
            ));
        }

        let mut left =
            self.without_context(Context::DisallowConditionalTypes, Parser::parse_ts_basic_type)?;

        while !self.cur_token().is_on_new_line && self.eat(Kind::LBrack) {
            if self.eat(Kind::RBrack) {
                // test ts ts_array_type
                // type A = string[];
                // type B = { a: number } [];
                left = self.ast.ts_array_type(self.end_span(span), left);
            } else {
                // test ts ts_indexed_access_type
                // type A = string[number];
                // type B = string[number][number][number][];
                let index_type = self.parse_ts_type()?;
                self.expect(Kind::RBrack)?;
                left = self.ast.ts_indexed_access_type(self.end_span(span), left, index_type);
            }
        }

        Ok(left)
    }

    // test ts ts_predefined_type
    // type A = any
    // type B = number;
    // type C = object;
    // type D = boolean;
    // type E = bigint;
    // type F = string;
    // type G = symbol;
    // type H = void;
    // type I = undefined;
    // type J = null;
    // type K = never
    fn parse_ts_basic_type(&mut self) -> Result<TSType<'a>> {
        match self.cur_kind() {
            Kind::LParen => {
                self.bump_any();
                let result = self.parse_ts_type();
                self.expect(Kind::RParen)?;
                result
            }
            Kind::LBrack => self.parse_ts_tuple_type(),
            Kind::LCurly => {
                if self.is_at_mapped_type() {
                    self.parse_ts_mapped_type()
                } else {
                    self.parse_ts_object_ype()
                }
            }
            Kind::Void => {
                let span = self.start_span();
                self.bump_any();
                Ok(self.ast.ts_void_keyword(self.end_span(span)))
            }
            Kind::This => {
                let span = self.start_span();
                self.bump_any();
                Ok(self.ast.ts_this_keyword(self.end_span(span)))
            }
            Kind::NoSubstitutionTemplate | Kind::TemplateHead => {
                self.parse_ts_template_literal_type(false)
            }
            Kind::Typeof => {
                if self.peek_at(Kind::Import) {
                    self.parse_ts_import_type()
                } else {
                    self.parse_ts_typeof_type()
                }
            }
            Kind::Import => self.parse_ts_import_type(),
            Kind::Minus if self.peek_kind().is_number() => self.parse_ts_literal_type(),
            Kind::Question => self.parse_js_doc_unknown_or_nullable_type(),
            kind if kind.is_literal() => self.parse_ts_literal_type(),
            _ => {
                if !self.peek_at(Kind::Dot) {
                    let keyword = self.parse_ts_keyword_type();
                    if let Some(keyword) = keyword {
                        return Ok(keyword);
                    }
                }
                self.parse_ts_reference_type()
            }
        }
    }

    fn parse_ts_keyword_type(&mut self) -> Option<TSType<'a>> {
        let span = self.start_span();
        match self.cur_kind() {
            Kind::Any => {
                self.bump_any();
                Some(self.ast.ts_any_keyword(self.end_span(span)))
            }
            Kind::Unknown => {
                self.bump_any();
                Some(self.ast.ts_unknown_keyword(self.end_span(span)))
            }
            Kind::Number => {
                self.bump_any();
                Some(self.ast.ts_number_keyword(self.end_span(span)))
            }
            Kind::Boolean => {
                self.bump_any();
                Some(self.ast.ts_boolean_keyword(self.end_span(span)))
            }
            Kind::Object => {
                self.bump_any();
                Some(self.ast.ts_object_keyword(self.end_span(span)))
            }
            Kind::String => {
                self.bump_any();
                Some(self.ast.ts_string_keyword(self.end_span(span)))
            }
            Kind::BigInt => {
                self.bump_any();
                Some(self.ast.ts_bigint_keyword(self.end_span(span)))
            }
            Kind::Symbol => {
                self.bump_any();
                Some(self.ast.ts_symbol_keyword(self.end_span(span)))
            }
            Kind::Null => {
                self.bump_any();
                Some(self.ast.ts_null_keyword(self.end_span(span)))
            }
            Kind::Undefined => {
                self.bump_any();
                Some(self.ast.ts_undefined_keyword(self.end_span(span)))
            }
            Kind::Never => {
                self.bump_any();
                Some(self.ast.ts_never_keyword(self.end_span(span)))
            }
            _ => None,
        }
    }

    // test ts ts_reference_type
    // type C = A;
    // type D = B.a;
    // type E = D.c.b.a;
    fn parse_ts_reference_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let type_name = self.parse_ts_qualified_name()?;
        let type_parameters =
            if self.cur_token().is_on_new_line { None } else { self.parse_ts_type_arguments()? };

        Ok(self.ast.ts_type_reference(self.end_span(span), type_name, type_parameters))
    }

    fn parse_ts_implement_name(&mut self) -> Result<Box<'a, TSClassImplements<'a>>> {
        let span = self.start_span();
        let expression = self.parse_ts_qualified_name()?;
        let type_parameters =
            if self.cur_token().is_on_new_line { None } else { self.parse_ts_type_arguments()? };

        Ok(self.ast.ts_type_implement(self.end_span(span), expression, type_parameters))
    }

    pub fn parse_ts_qualified_name(&mut self) -> Result<TSTypeName<'a>> {
        let span = self.start_span();
        let identifier_name = self.parse_identifier_name()?;
        let mut left = TSTypeName::IdentifierName(self.ast.alloc(identifier_name));

        while self.eat(Kind::Dot) {
            let right = self.parse_identifier_name()?;
            left = TSTypeName::QualifiedName(self.ast.alloc(TSQualifiedName {
                span: self.end_span(span),
                left,
                right,
            }));
        }

        Ok(left)
    }

    pub fn parse_ts_type_arguments(
        &mut self,
    ) -> Result<Option<Box<'a, TSTypeParameterInstantiation<'a>>>> {
        self.re_lex_ts_l_angle();
        if !self.at(Kind::LAngle) {
            return Ok(None);
        }
        let span = self.start_span();
        let params = TSTypeArgumentList::parse(self)?.params;
        Ok(Some(self.ast.ts_type_arguments(self.end_span(span), params)))
    }

    pub fn parse_ts_type_arguments_in_expression(
        &mut self,
    ) -> Result<Option<Box<'a, TSTypeParameterInstantiation<'a>>>> {
        if !matches!(self.cur_kind(), Kind::LAngle | Kind::ShiftLeft) {
            return Ok(None);
        }
        let span = self.start_span();

        Ok(self
            .try_parse(|p| {
                p.re_lex_ts_l_angle();

                let params = TSTypeArgumentList::parse(p)?.params;
                if p.cur_kind().can_follow_type_arguments_in_expr() {
                    Ok(params)
                } else {
                    p.unexpected()
                }
            })
            .ok()
            .map(|types| self.ast.ts_type_arguments(self.end_span(span), types)))
    }

    fn parse_ts_tuple_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let elements = TSTupleElementList::parse(self)?.elements;
        Ok(self.ast.ts_tuple_type(self.end_span(span), elements))
    }

    fn is_at_function_type(&mut self) -> bool {
        if self.at(Kind::LAngle) {
            return true;
        }

        if !self.at(Kind::LParen) {
            return false;
        }

        let checkpoint = self.checkpoint();

        self.bump_any(); // bump (

        if self.at(Kind::RParen) || self.at(Kind::Dot3) {
            self.rewind(checkpoint);
            return true;
        }

        let mut is_function_parameter_start =
            self.at(Kind::This) || self.cur_kind().is_binding_identifier();

        if is_function_parameter_start {
            self.bump_any();
        }

        if match self.cur_kind() {
            Kind::LBrack => ArrayPatternList::parse(self).is_ok(),
            Kind::LCurly => ObjectPatternProperties::parse(self).is_ok(),
            _ => false,
        } {
            is_function_parameter_start = true;
        }

        let result = if is_function_parameter_start {
            matches!(self.cur_kind(), Kind::Colon | Kind::Eq | Kind::Comma | Kind::Question)
                || (self.at(Kind::RParen) && self.peek_at(Kind::Arrow))
        } else {
            false
        };

        self.rewind(checkpoint);

        result
    }

    fn is_at_mapped_type(&mut self) -> bool {
        if !self.at(Kind::LCurly) {
            return false;
        }

        if self.peek_at(Kind::Plus) || self.peek_at(Kind::Minus) {
            return self.nth_at(2, Kind::Readonly);
        }

        let mut offset = 1;

        if self.nth_at(offset, Kind::Readonly) {
            offset += 1;
        }

        self.nth_at(offset, Kind::LBrack)
            && self.nth_kind(offset + 1).is_identifier_name()
            && self.nth_at(offset + 2, Kind::In)
    }

    fn parse_ts_mapped_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        self.expect(Kind::LCurly)?;
        let mut readonly = TSMappedTypeModifierOperator::None;
        if self.eat(Kind::Readonly) {
            readonly = TSMappedTypeModifierOperator::True;
        } else if self.eat(Kind::Plus) && self.eat(Kind::Readonly) {
            readonly = TSMappedTypeModifierOperator::Plus;
        } else if self.eat(Kind::Minus) && self.eat(Kind::Readonly) {
            readonly = TSMappedTypeModifierOperator::Minus;
        }

        self.expect(Kind::LBrack)?;
        let type_parameter_span = self.start_span();
        if !self.cur_kind().is_identifier_name() {
            return self.unexpected();
        }
        let name = self.parse_binding_identifier()?;
        self.expect(Kind::In)?;
        let constraint = self.parse_ts_type()?;
        let type_parameter = self.ast.ts_type_parameter(
            self.end_span(type_parameter_span),
            name,
            Some(constraint),
            None,
            false,
            false,
        );

        let name_type = if self.eat(Kind::As) { Some(self.parse_ts_type()?) } else { None };
        self.expect(Kind::RBrack)?;

        let optional = match self.cur_kind() {
            Kind::Question => {
                self.bump_any();
                TSMappedTypeModifierOperator::True
            }
            Kind::Minus => {
                self.bump_any();
                self.expect(Kind::Question)?;
                TSMappedTypeModifierOperator::Minus
            }
            Kind::Plus => {
                self.bump_any();
                self.expect(Kind::Question)?;
                TSMappedTypeModifierOperator::Plus
            }
            _ => TSMappedTypeModifierOperator::None,
        };

        self.expect(Kind::Colon)?;
        let type_annotation = self.parse_ts_type()?;

        self.bump(Kind::Semicolon);
        self.expect(Kind::RCurly)?;

        Ok(self.ast.ts_mapped_type(
            self.end_span(span),
            type_parameter,
            name_type,
            type_annotation,
            optional,
            readonly,
        ))
    }

    pub fn is_at_named_tuple_element(&mut self) -> bool {
        let offset = u8::from(self.at(Kind::Dot3));
        let has_colon = self.nth_at(offset + 1, Kind::Colon);
        let has_question_colon =
            self.nth_at(offset + 1, Kind::Question) && self.nth_at(offset + 2, Kind::Colon);

        self.nth_kind(offset).is_identifier_name() && (has_colon || has_question_colon)
    }

    fn parse_ts_object_ype(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let mut member_list = TSInterfaceOrObjectBodyList::new(self);
        member_list.parse(self)?;

        Ok(self.ast.ts_type_literal(self.end_span(span), member_list.body))
    }

    fn parse_ts_literal_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let negative = self.eat(Kind::Minus);

        let expression = self.parse_literal_expression()?;

        let span = self.end_span(span);
        let literal = if negative {
            match self.ast.unary_expression(span, UnaryOperator::UnaryNegation, true, expression) {
                Expression::UnaryExpression(unary_expr) => TSLiteral::UnaryExpression(unary_expr),
                _ => unreachable!(),
            }
        } else {
            match expression {
                Expression::BooleanLiteral(literal) => TSLiteral::BooleanLiteral(literal),
                Expression::NullLiteral(literal) => TSLiteral::NullLiteral(literal),
                Expression::NumberLiteral(literal) => TSLiteral::NumberLiteral(literal),
                Expression::BigintLiteral(literal) => TSLiteral::BigintLiteral(literal),
                Expression::RegExpLiteral(literal) => TSLiteral::RegExpLiteral(literal),
                Expression::StringLiteral(literal) => TSLiteral::StringLiteral(literal),
                Expression::TemplateLiteral(literal) => TSLiteral::TemplateLiteral(literal),
                _ => return self.unexpected(),
            }
        };

        Ok(self.ast.ts_literal_type(span, literal))
    }

    fn parse_ts_template_literal_type(&mut self, tagged: bool) -> Result<TSType<'a>> {
        let span = self.start_span();
        let mut types = self.ast.new_vec();
        let mut quasis = self.ast.new_vec();
        match self.cur_kind() {
            Kind::NoSubstitutionTemplate => {
                quasis.push(self.parse_template_element(tagged));
            }
            Kind::TemplateHead => {
                quasis.push(self.parse_template_element(tagged));
                types.push(self.parse_ts_type()?);
                self.re_lex_template_substitution_tail();
                loop {
                    match self.cur_kind() {
                        Kind::Eof => self.expect(Kind::TemplateTail)?,
                        Kind::TemplateTail => {
                            quasis.push(self.parse_template_element(tagged));
                            break;
                        }
                        Kind::TemplateMiddle => {
                            quasis.push(self.parse_template_element(tagged));
                        }
                        _ => {
                            types.push(self.parse_ts_type()?);
                            self.re_lex_template_substitution_tail();
                        }
                    }
                }
            }
            _ => unreachable!("parse_template_literal"),
        }

        Ok(self.ast.ts_template_literal_type(self.end_span(span), quasis, types))
    }

    fn parse_ts_typeof_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        self.expect(Kind::Typeof)?;
        let expr_name = self.parse_ts_qualified_name()?;
        let type_parameters = self.parse_ts_type_arguments()?;
        Ok(self.ast.ts_type_query_type(self.end_span(span), expr_name, type_parameters))
    }

    fn parse_ts_import_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let is_type_of = self.eat(Kind::Typeof);
        self.expect(Kind::Import)?;
        self.expect(Kind::LParen)?;
        let parameter = self.parse_ts_type()?;
        self.expect(Kind::RParen)?;

        let qualifier =
            if self.eat(Kind::Dot) { Some(self.parse_ts_qualified_name()?) } else { None };

        let type_parameters = self.parse_ts_type_arguments()?;

        Ok(self.ast.ts_import_type(
            self.end_span(span),
            is_type_of,
            parameter,
            qualifier,
            type_parameters,
        ))
    }

    fn parse_ts_constructor_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let r#abstract = self.eat(Kind::Abstract);
        self.expect(Kind::New)?;
        let type_parameters = self.parse_ts_type_parameters()?;
        let params = self.parse_formal_parameters(FormalParameterKind::Signature)?;
        self.expect(Kind::Arrow)?;
        let return_type_span = self.start_span();
        let return_type = self.parse_ts_return_type()?;
        let return_type = self.ast.ts_type_annotation(self.end_span(return_type_span), return_type);

        Ok(self.ast.ts_constructor_type(
            self.end_span(span),
            r#abstract,
            params,
            return_type,
            type_parameters,
        ))
    }

    fn parse_ts_function_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let type_parameters = self.parse_ts_type_parameters()?;
        let params = self.parse_formal_parameters(FormalParameterKind::Signature)?;
        self.expect(Kind::Arrow)?;
        let return_type_span = self.start_span();
        let return_type = self.parse_ts_return_type()?;
        let return_type = self.ast.ts_type_annotation(self.end_span(return_type_span), return_type);
        Ok(self.ast.ts_function_type(self.end_span(span), params, return_type, type_parameters))
    }

    fn parse_ts_infer_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        self.expect(Kind::Infer)?;

        let parameter_span = self.start_span();
        let name = self.parse_binding_identifier()?;

        let constraint = self.try_parse(Parser::parse_constraint_of_infer_type).unwrap_or(None);

        let type_parameter = self.ast.ts_type_parameter(
            self.end_span(parameter_span),
            name,
            constraint,
            None,
            false,
            false,
        );

        Ok(self.ast.ts_infer_type(self.end_span(span), type_parameter))
    }

    fn parse_constraint_of_infer_type(&mut self) -> Result<Option<TSType<'a>>> {
        if self.eat(Kind::Extends) {
            let constraint =
                self.with_context(Context::DisallowConditionalTypes, Self::parse_ts_type)?;
            if self.ctx.has_disallow_conditional_types() || !self.at(Kind::Question) {
                return Ok(Some(constraint));
            }
        }
        self.unexpected()
    }

    pub fn parse_ts_return_type_annotation(
        &mut self,
    ) -> Result<Option<Box<'a, TSTypeAnnotation<'a>>>> {
        if !self.ts_enabled() {
            return Ok(None);
        }
        if !self.at(Kind::Colon) {
            return Ok(None);
        }
        let span = self.start_span();
        self.bump_any(); // bump colon
        let return_type = self.parse_ts_return_type()?;
        Ok(Some(self.ast.ts_type_annotation(self.end_span(span), return_type)))
    }

    fn parse_ts_type_predicate(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        let asserts = self.eat(Kind::Asserts);

        let parameter_name = if self.at(Kind::This) {
            let span = self.start_span();
            self.bump_any();
            TSTypePredicateName::This(TSThisKeyword { span: self.end_span(span) })
        } else {
            TSTypePredicateName::Identifier(self.parse_identifier_name()?)
        };

        if !asserts {
            self.expect(Kind::Is)?;
        } else if !self.eat(Kind::Is) {
            return Ok(self.ast.ts_type_predicate(
                self.end_span(span),
                parameter_name,
                asserts,
                None,
            ));
        }

        let type_span = self.start_span();
        let type_annotation = self.parse_ts_type()?;
        let type_annotation =
            Some(self.ast.ts_type_annotation(self.end_span(type_span), type_annotation));

        Ok(self.ast.ts_type_predicate(
            self.end_span(span),
            parameter_name,
            asserts,
            type_annotation,
        ))
    }

    pub fn parse_ts_return_type(&mut self) -> Result<TSType<'a>> {
        let asserts = self.at(Kind::Asserts)
            && (self.peek_kind().is_identifier() || self.peek_at(Kind::This));
        let is_predicate =
            (self.cur_kind().is_identifier() || self.at(Kind::This)) && self.peek_at(Kind::Is);
        if !self.peek_token().is_on_new_line && (asserts || is_predicate) {
            self.parse_ts_type_predicate()
        } else {
            self.without_context(Context::DisallowConditionalTypes, Self::parse_ts_type)
        }
    }

    pub fn is_next_at_type_member_name(&mut self) -> bool {
        self.peek_kind().is_literal_property_name() || self.peek_at(Kind::LBrack)
    }

    pub fn parse_ts_call_signature_member(&mut self) -> Result<TSSignature<'a>> {
        let span = self.start_span();
        let type_parameters = self.parse_ts_type_parameters()?;
        let params = self.parse_formal_parameters(FormalParameterKind::Signature)?;
        let return_type = self.parse_ts_return_type_annotation()?;
        self.bump(Kind::Comma);
        self.bump(Kind::Semicolon);
        Ok(self.ast.ts_call_signature_declaration(
            self.end_span(span),
            params,
            return_type,
            type_parameters,
        ))
    }

    pub fn parse_ts_getter_signature_member(&mut self) -> Result<TSSignature<'a>> {
        let span = self.start_span();
        self.expect(Kind::Get)?;
        let (key, computed) = self.parse_property_name()?;
        let params = self.parse_formal_parameters(FormalParameterKind::Signature)?;
        let return_type = self.parse_ts_return_type_annotation()?;
        self.bump(Kind::Comma);
        self.bump(Kind::Semicolon);
        if !params.is_empty() {
            self.error(diagnostics::GetterParameters(params.span));
        }
        Ok(self.ast.ts_method_signature(
            self.end_span(span),
            key,
            computed,
            /* optional */ false,
            TSMethodSignatureKind::Get,
            params,
            return_type,
            None,
        ))
    }

    pub fn parse_ts_setter_signature_member(&mut self) -> Result<TSSignature<'a>> {
        let span = self.start_span();
        self.expect(Kind::Set)?;
        let (key, computed) = self.parse_property_name()?;
        let params = self.parse_formal_parameters(FormalParameterKind::Signature)?;
        let return_type = self.parse_ts_return_type_annotation()?;
        self.bump(Kind::Comma);
        self.bump(Kind::Semicolon);
        if params.items.len() != 1 {
            self.error(diagnostics::SetterParameters(params.span));
        }
        if let Some(return_type) = return_type.as_ref() {
            self.error(diagnostics::ASetAccessorCannotHaveAReturnTypeAnnotation(return_type.span));
        }
        Ok(self.ast.ts_method_signature(
            self.end_span(span),
            key,
            computed,
            /* optional */ false,
            TSMethodSignatureKind::Set,
            params,
            return_type,
            None,
        ))
    }

    pub fn parse_ts_property_or_method_signature_member(&mut self) -> Result<TSSignature<'a>> {
        let span = self.start_span();
        let readonly = self.at(Kind::Readonly) && self.is_next_at_type_member_name();

        if readonly {
            self.bump_any();
        }

        let (key, computed) = self.parse_property_name()?;
        let optional = self.eat(Kind::Question);

        if self.at(Kind::LParen) || self.at(Kind::LAngle) {
            let TSSignature::TSCallSignatureDeclaration(call_signature) = self.parse_ts_call_signature_member()? else { unreachable!() };
            self.bump(Kind::Comma);
            self.bump(Kind::Semicolon);
            let call_signature = Box::into_inner(call_signature);
            Ok(self.ast.ts_method_signature(
                self.end_span(span),
                key,
                computed,
                optional,
                TSMethodSignatureKind::Method,
                call_signature.params,
                call_signature.return_type,
                call_signature.type_parameters,
            ))
        } else {
            let type_annotation = self.parse_ts_type_annotation()?;
            self.bump(Kind::Comma);
            self.bump(Kind::Semicolon);
            Ok(self.ast.ts_property_signature(
                self.end_span(span),
                computed,
                optional,
                readonly,
                key,
                type_annotation,
            ))
        }
    }

    pub fn parse_ts_constructor_signature_member(&mut self) -> Result<TSSignature<'a>> {
        let span = self.start_span();
        self.expect(Kind::New)?;

        let type_parameters = self.parse_ts_type_parameters()?;
        let params = self.parse_formal_parameters(FormalParameterKind::Signature)?;
        let return_type = self.parse_ts_return_type_annotation()?;
        self.bump(Kind::Comma);
        self.bump(Kind::Semicolon);

        Ok(self.ast.ts_construct_signature_declaration(
            self.end_span(span),
            params,
            return_type,
            type_parameters,
        ))
    }

    pub fn parse_ts_index_signature_member(&mut self) -> Result<TSSignature<'a>> {
        let span = self.start_span();
        while self.is_nth_at_modifier(0, false) {
            if !self.eat(Kind::Readonly) {
                self.unexpected()?;
            }
        }

        self.bump(Kind::LBrack);
        let index_name = self.parse_ts_index_signature_name()?;
        let mut parameters = self.ast.new_vec();
        parameters.push(index_name);
        self.expect(Kind::RBrack)?;

        let type_annotation = self.parse_ts_type_annotation()?;
        if let Some(type_annotation) = type_annotation {
            self.bump(Kind::Comma);
            self.bump(Kind::Semicolon);
            Ok(self.ast.ts_index_signature(self.end_span(span), parameters, type_annotation))
        } else {
            self.unexpected()
        }
    }

    fn parse_ts_index_signature_name(&mut self) -> Result<Box<'a, TSIndexSignatureName<'a>>> {
        let span = self.start_span();
        let name = self.parse_identifier_name()?.name;
        let type_annotation = self.parse_ts_type_annotation()?;

        if type_annotation.is_none() {
            self.unexpected()?;
        }

        Ok(self.ast.alloc(TSIndexSignatureName {
            span: self.end_span(span),
            name,
            type_annotation: type_annotation.unwrap(),
        }))
    }

    pub fn parse_class_element_modifiers(
        &mut self,
        is_constructor_parameter: bool,
    ) -> ModifierFlags {
        let mut flags = ModifierFlags::empty();

        if !self.ts_enabled() {
            return flags;
        }

        loop {
            if !self.is_nth_at_modifier(0, is_constructor_parameter) {
                break;
            }

            if let Ok(modifier_flag) = self.cur_kind().try_into() {
                flags.set(modifier_flag, true);
            } else {
                break;
            }

            self.bump_any();
        }

        flags
    }

    fn parse_js_doc_unknown_or_nullable_type(&mut self) -> Result<TSType<'a>> {
        let span = self.start_span();
        self.bump_any(); // bump `?`
        let type_annotation = self.parse_ts_type()?;
        let span = self.end_span(span);
        if matches!(
            self.cur_kind(),
            Kind::Comma | Kind::RCurly | Kind::RParen | Kind::RAngle | Kind::Eq | Kind::Pipe
        ) {
            Ok(self.ast.js_doc_unknown_type(span))
        } else {
            Ok(self.ast.js_doc_nullable_type(span, type_annotation, /* postfix */ false))
        }
    }
}
