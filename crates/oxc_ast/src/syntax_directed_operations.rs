//! [Syntax-Directed Operations](https://tc39.es/ecma262/#sec-syntax-directed-operations)

#[allow(clippy::wildcard_imports)]
use crate::{ast::*, Span};

/// [`BoundName`](https://tc39.es/ecma262/#sec-static-semantics-boundnames)
pub trait BoundName {
    fn bound_name(&self) -> Option<&BindingIdentifier>;
}

pub trait BoundNames {
    fn bound_names(&self) -> Vec<&BindingIdentifier>;
}

impl<'a> BoundNames for BindingPattern<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        match &self.kind {
            BindingPatternKind::BindingIdentifier(ident) => ident.bound_names(),
            BindingPatternKind::ArrayPattern(array) => array.bound_names(),
            BindingPatternKind::ObjectPattern(object) => object.bound_names(),
            BindingPatternKind::AssignmentPattern(assignment) => assignment.bound_names(),
            BindingPatternKind::RestElement(rest) => rest.argument.bound_names(),
        }
    }
}

impl<'a> BoundNames for Option<BindingPattern<'a>> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.as_ref().map_or(vec![], BoundNames::bound_names)
    }
}

impl BoundNames for Option<BindingIdentifier> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.as_ref().map_or(vec![], |ident| vec![ident])
    }
}

impl BoundNames for BindingIdentifier {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        vec![self]
    }
}

impl<'a> BoundNames for ArrayPattern<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.elements.iter().flat_map(BoundNames::bound_names).collect()
    }
}

impl<'a> BoundNames for ObjectPattern<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.properties
            .iter()
            .flat_map(|p| match p {
                ObjectPatternProperty::Property(property) => {
                    if let PropertyValue::Pattern(pattern) = &property.value {
                        pattern.bound_names()
                    } else {
                        vec![]
                    }
                }
                ObjectPatternProperty::RestElement(rest) => rest.argument.bound_names(),
            })
            .collect()
    }
}

impl<'a> BoundNames for AssignmentPattern<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.left.bound_names()
    }
}

impl<'a> BoundNames for RestElement<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.argument.bound_names()
    }
}

impl<'a> BoundNames for FormalParameters<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.items.iter().flat_map(BoundNames::bound_names).collect()
    }
}

impl<'a> BoundNames for Declaration<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        match self {
            Declaration::VariableDeclaration(decl) => decl.bound_names(),
            Declaration::FunctionDeclaration(func) => func.bound_names(),
            Declaration::ClassDeclaration(decl) => decl.bound_names(),
            _ => vec![],
        }
    }
}

impl<'a> BoundNames for VariableDeclaration<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.declarations.iter().flat_map(|declarator| declarator.id.bound_names()).collect()
    }
}

impl<'a> BoundName for Function<'a> {
    fn bound_name(&self) -> Option<&BindingIdentifier> {
        self.id.as_ref()
    }
}

impl<'a> BoundNames for Function<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.bound_name().map_or(vec![], |name| vec![name])
    }
}

impl<'a> BoundName for Class<'a> {
    fn bound_name(&self) -> Option<&BindingIdentifier> {
        self.id.as_ref()
    }
}

impl<'a> BoundNames for Class<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.bound_name().map_or(vec![], |name| vec![name])
    }
}

impl<'a> BoundNames for FormalParameter<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.pattern.bound_names()
    }
}

impl<'a> BoundNames for ModuleDeclaration<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        match &self.kind {
            ModuleDeclarationKind::ImportDeclaration(decl) => decl.bound_names(),
            // ModuleDeclarationKind::ExportNamedDeclaration(decl) => decl.bound_names(),
            _ => vec![],
        }
    }
}

impl<'a> BoundNames for ImportDeclaration<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.specifiers
            .iter()
            .map(|specifier| match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => &specifier.local,
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => &specifier.local,
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => &specifier.local,
            })
            .collect()
    }
}

impl<'a> BoundNames for ExportNamedDeclaration<'a> {
    fn bound_names(&self) -> Vec<&BindingIdentifier> {
        self.declaration.as_ref().map_or(vec![], BoundNames::bound_names)
    }
}

/// [`IsSimpleParameterList`](https://tc39.es/ecma262/#sec-static-semantics-issimpleparameterlist)
pub trait IsSimpleParameterList {
    fn is_simple_parameter_list(&self) -> bool;
}

impl<'a> IsSimpleParameterList for FormalParameters<'a> {
    fn is_simple_parameter_list(&self) -> bool {
        self.items
            .iter()
            .all(|pat| matches!(pat.pattern.kind, BindingPatternKind::BindingIdentifier(_)))
    }
}

/// [`PropName`](https://tc39.es/ecma262/#sec-static-semantics-propname)
pub trait PropName {
    fn prop_name(&self) -> Option<(&str, Span)>;
}

impl<'a> PropName for ObjectProperty<'a> {
    fn prop_name(&self) -> Option<(&str, Span)> {
        match self {
            ObjectProperty::Property(prop) => prop.prop_name(),
            ObjectProperty::SpreadProperty(_) => None,
        }
    }
}

impl<'a> PropName for Property<'a> {
    fn prop_name(&self) -> Option<(&str, Span)> {
        if self.kind != PropertyKind::Init || self.method || self.shorthand || self.computed {
            return None;
        }
        self.key.prop_name()
    }
}

impl<'a> PropName for PropertyKey<'a> {
    fn prop_name(&self) -> Option<(&str, Span)> {
        match self {
            PropertyKey::Identifier(ident) => Some((&ident.name, ident.span)),
            PropertyKey::PrivateIdentifier(_) => None,
            PropertyKey::Expression(expr) => match &expr {
                Expression::Identifier(ident) => Some((&ident.name, ident.span)),
                Expression::StringLiteral(lit) => Some((&lit.value, lit.span)),
                _ => None,
            },
        }
    }
}

impl<'a> PropName for ClassElement<'a> {
    fn prop_name(&self) -> Option<(&str, Span)> {
        match self {
            ClassElement::MethodDefinition(def) => def.prop_name(),
            ClassElement::TSAbstractMethodDefinition(def) => def.method_definition.prop_name(),
            ClassElement::PropertyDefinition(def) => def.prop_name(),
            ClassElement::TSAbstractPropertyDefinition(def) => def.property_definition.prop_name(),
            _ => None,
        }
    }
}

impl<'a> PropName for MethodDefinition<'a> {
    fn prop_name(&self) -> Option<(&str, Span)> {
        if self.computed {
            return None;
        }
        self.key.prop_name()
    }
}

impl<'a> PropName for PropertyDefinition<'a> {
    fn prop_name(&self) -> Option<(&str, Span)> {
        if self.computed {
            return None;
        }
        self.key.prop_name()
    }
}

/// [`PrivateBoundIdentifiers`](https://tc39.es/ecma262/#sec-static-semantics-privateboundidentifiers)
pub trait PrivateBoundIdentifiers {
    fn private_bound_identifiers(&self) -> Option<PrivateIdentifier>;
}

impl<'a> PrivateBoundIdentifiers for ClassElement<'a> {
    fn private_bound_identifiers(&self) -> Option<PrivateIdentifier> {
        match self {
            ClassElement::StaticBlock(_) | ClassElement::TSIndexSignature(_) => None,
            ClassElement::MethodDefinition(def) => def.private_bound_identifiers(),
            ClassElement::PropertyDefinition(def) => def.private_bound_identifiers(),
            ClassElement::AccessorProperty(def) => def.private_bound_identifiers(),
            ClassElement::TSAbstractMethodDefinition(def) => {
                def.method_definition.private_bound_identifiers()
            }
            ClassElement::TSAbstractPropertyDefinition(def) => {
                def.property_definition.private_bound_identifiers()
            }
        }
    }
}

impl<'a> PrivateBoundIdentifiers for MethodDefinition<'a> {
    fn private_bound_identifiers(&self) -> Option<PrivateIdentifier> {
        self.value.body.as_ref()?;
        if let PropertyKey::PrivateIdentifier(ident) = &self.key {
            return Some((**ident).clone());
        }
        None
    }
}

impl<'a> PrivateBoundIdentifiers for PropertyDefinition<'a> {
    fn private_bound_identifiers(&self) -> Option<PrivateIdentifier> {
        if let PropertyKey::PrivateIdentifier(ident) = &self.key {
            return Some((**ident).clone());
        }
        None
    }
}

impl<'a> PrivateBoundIdentifiers for AccessorProperty<'a> {
    fn private_bound_identifiers(&self) -> Option<PrivateIdentifier> {
        if let PropertyKey::PrivateIdentifier(ident) = &self.key {
            return Some((**ident).clone());
        }
        None
    }
}
