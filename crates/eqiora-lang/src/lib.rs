//! **eqiora-lang** — the specification-first Eqiora Language frontend.
//!
//! The lexer retains every byte range, including whitespace and comments.
//! The parser produces a source-oriented AST and structured diagnostics; it
//! never constructs Semantic Kernel nodes directly. Typed transaction
//! lowering belongs to the compiler layer so syntax remains independent of
//! graph storage.

mod ast;
mod ast_property;
mod cartesian;
mod decimal;
mod draft;
mod draft_spatial;
mod factory;
mod formatter;
mod lexer;
mod parser;

pub use ast::{
    ActivationSyntax, BinaryOp, BoundaryConnectionDecl, BoundaryDecl, BoundaryFamilyBinderSyntax,
    BoundaryPairingSyntax, BoundaryPortReferenceSyntax, BoundaryPortSelectorSyntax,
    BoundarySetBindingDecl, BoundarySetMemberSyntax, BoundarySideSyntax, ClockBindingDecl,
    ClockDecl, ClockRequirementDecl, ComponentDecl, ComponentItem, ComponentParameterDecl,
    ComponentPortDecl, ComponentPortFamilyDecl, ConnectionDecl, ConnectionSyntax, ConnectorDecl,
    ConnectorQuantitySyntax, ConnectorSyntax, DocComment, Document, DomainDecl, DomainSyntax,
    Equation, ExactIntegerSyntax, Expr, ExprKind, FieldBindingDecl, FieldDecl, FieldRoleSyntax,
    FrameSyntax, InitialDecl, InstanceDecl, Item, LetDecl, ModelDecl, NamePath,
    ParameterBindingDecl, ParameterDecl, PortDecl, PortSyntax, PureOperatorBinaryOp,
    PureOperatorDecl, PureOperatorExpr, PureOperatorExprKind, PureOperatorFormal,
    PureValueClassSyntax, RelationDecl, RelationFamilyDecl, SignalDirectionSyntax,
    SupportBindingDecl, SupportSlotDecl, SupportSlotSyntax, TextRange, UnaryOp, ValueShapeSyntax,
    ValueTypeSyntax, ValueTypeSyntaxKind, VisibilitySyntax,
};
pub use cartesian::CartesianCoordinateSyntax;
pub use decimal::DecimalLiteral;
pub use draft::{
    DraftConservingConnection, DraftConservingPort, DraftDeclaration, DraftExpression, DraftField,
    DraftParameter, DraftPhysicalDomain, DraftRelation, ModelDraft, NativeModelAst,
};
pub use draft_spatial::DraftSpatialDomain;
pub use factory::{AstConstructionError, SourceAstFactory};
pub use formatter::format;
pub use lexer::{LexResult, Token, TokenKind, lex};
pub use parser::{ParseResult, parse};
