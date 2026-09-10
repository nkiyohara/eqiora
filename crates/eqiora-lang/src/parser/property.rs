use super::Parser;
use crate::ast::{TextRange, VisibilitySyntax};
use crate::ast_property::{
    MaterialCompositionDecl, PropertyBindingDecl, PropertyContractDecl, PropertyReleaseDecl,
};
use crate::lexer::{Token, TokenKind};

impl Parser<'_> {
    pub(super) fn following_significant_token(&self) -> Option<&Token> {
        self.tokens[self.cursor.saturating_add(1)..]
            .iter()
            .find(|token| !token.kind().is_trivia())
    }
    pub(super) fn previous_significant_range(&self) -> TextRange {
        self.tokens[..self.cursor]
            .iter()
            .rev()
            .find(|token| !token.kind().is_trivia())
            .map_or(TextRange::new(0, 0), Token::range)
    }

    pub(super) fn parse_top_property(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
        models_started: bool,
        contracts: &mut Vec<PropertyContractDecl>,
        releases: &mut Vec<PropertyReleaseDecl>,
    ) {
        if models_started {
            self.error_here("property declarations must precede model declarations");
        }
        self.bump();
        let parsed = if self.at_keyword("contract") {
            self.parse_property_contract(start, visibility)
                .map(|value| contracts.push(value))
        } else if self.at_keyword("release") {
            self.parse_property_release(start, visibility)
                .map(|value| releases.push(value))
        } else {
            self.error_here("expected `contract` or `release` after `property`");
            None
        };
        if parsed.is_none() {
            self.recover_top_level();
        }
    }

    pub(super) fn parse_property_contract(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<PropertyContractDecl> {
        self.expect_keyword("contract")?;
        let name = self
            .declaration_name("property contract name")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftParen, "`(` before property inputs")?;
        let mut inputs = Vec::new();
        while !self.at(TokenKind::RightParen) {
            self.expect_keyword("input")?;
            let input = self
                .declaration_name("independent property input")?
                .text()
                .to_owned();
            self.expect(TokenKind::Colon, "`:` before property input type")?;
            inputs.push((input, self.parse_value_type()?));
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RightParen, "`)` after property inputs")?;
        self.expect(TokenKind::Colon, "`:` before property result type")?;
        let value_type = self.parse_value_type()?;
        self.expect(TokenKind::LeftBrace, "`{` before property profile")?;
        self.expect_keyword("derivatives")?;
        let derivatives = if self.at_keyword("first_partials") {
            self.bump();
            eqiora_schema::kernel::PropertyDerivatives::FirstPartials
        } else if self.at_keyword("first_open_intervals") {
            self.bump();
            eqiora_schema::kernel::PropertyDerivatives::FirstOpenIntervals
        } else {
            self.expect_keyword("value_only")?;
            eqiora_schema::kernel::PropertyDerivatives::ValueOnly
        };
        self.expect(
            TokenKind::Semicolon,
            "`;` after property derivative profile",
        )?;
        let branch = if self.at_keyword("branch") {
            self.bump();
            let value = self.parse_name_path("property branch")?;
            self.expect(TokenKind::Semicolon, "`;` after property branch")?;
            Some(value)
        } else {
            None
        };
        let end = self
            .expect(TokenKind::RightBrace, "`}` after property contract")?
            .range()
            .end();
        Some(PropertyContractDecl {
            comments: Default::default(),
            visibility,
            name,
            value_type,
            inputs,
            derivatives,
            branch,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_property_release(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<PropertyReleaseDecl> {
        self.expect_keyword("release")?;
        let name = self
            .declaration_name("property release name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` before property contract")?;
        let contract = self.parse_name_path("property contract name")?;
        self.expect(TokenKind::LeftBrace, "`{` after property release contract")?;
        let (source_value, source_dimension, coherent_si_scale, validity) =
            if self.at_keyword("table") {
                let value = self.parse_property_table()?;
                let crate::PropertySourceSyntax::Table(table) = &value else {
                    unreachable!()
                };
                let dimension = table.value_dimension().clone();
                let scale = crate::SourceAstFactory::expression(
                    crate::ExprKind::Number(
                        crate::DecimalLiteral::parse("1").expect("canonical unit scale"),
                    ),
                    table.range(),
                )
                .expect("parsed table range");
                (value, Some(dimension), scale, None)
            } else {
                self.expect_keyword("analytic")?;
                self.expect(TokenKind::LeftBrace, "`{` after analytic")?;
                self.expect_keyword("value")?;
                self.expect(TokenKind::Equal, "`=` after analytic value")?;
                let value = self.parse_expression(0)?;
                self.expect(TokenKind::Semicolon, "`;` after analytic value")?;
                let (dimension, scale) = if self.at_keyword("source_unit") {
                    self.bump();
                    self.expect(TokenKind::Colon, "`:` before source unit dimension")?;
                    let dimension = self.parse_dimension_expression()?;
                    self.expect(TokenKind::Equal, "`=` before coherent-SI scale")?;
                    let scale = self.parse_expression(0)?;
                    self.expect(TokenKind::Semicolon, "`;` after source unit")?;
                    (Some(dimension), scale)
                } else {
                    (
                        None,
                        crate::SourceAstFactory::expression(
                            crate::ExprKind::Number(
                                crate::DecimalLiteral::parse("1").expect("canonical unit scale"),
                            ),
                            value.range(),
                        )
                        .expect("parsed value range"),
                    )
                };
                self.expect(TokenKind::RightBrace, "`}` after analytic body")?;
                self.expect_keyword("validity")?;
                let validity = self.parse_property_validity()?;
                self.expect(TokenKind::Semicolon, "`;` after validity")?;
                (
                    crate::PropertySourceSyntax::Expression(value),
                    dimension,
                    scale,
                    validity,
                )
            };
        self.expect_keyword("outside")?;
        self.expect_keyword("reject")?;
        self.expect(TokenKind::Semicolon, "`;` after outside-domain policy")?;
        self.expect_keyword("branch")?;
        let branch = Some(self.parse_name_path("release branch")?);
        self.expect(TokenKind::Semicolon, "`;` after release branch")?;
        self.expect_keyword("citation")?;
        let citation = self.parse_name_path("citation identity")?;
        self.expect(TokenKind::Semicolon, "`;` after citation")?;
        self.expect_keyword("license")?;
        let license = self.parse_name_path("license identity")?;
        self.expect(TokenKind::Semicolon, "`;` after license")?;
        let end = self
            .expect(TokenKind::RightBrace, "`}` after property release")?
            .range()
            .end();
        Some(PropertyReleaseDecl {
            comments: Default::default(),
            visibility,
            name,
            contract,
            source_value,
            validity,
            branch,
            source_dimension,
            coherent_si_scale,
            citation,
            license,
            range: TextRange::new(start, end),
        })
    }

    fn parse_property_validity(&mut self) -> Option<Option<crate::Expr>> {
        if self.at_keyword("unconditional") {
            self.bump();
            return Some(None);
        }
        if self
            .following_significant_token()
            .is_some_and(|token| token.text() == "in")
        {
            let token = self.declaration_name("validity input")?;
            let name = token.text().to_owned();
            let range = token.range();
            self.expect_keyword("in")?;
            self.expect(TokenKind::LeftBracket, "`[` before validity interval")?;
            let lower = self.parse_expression(0)?;
            self.expect(TokenKind::Comma, "`,` between validity endpoints")?;
            let upper = self.parse_expression(0)?;
            let end = self
                .expect(TokenKind::RightBracket, "`]` after validity interval")?
                .range()
                .end();
            let formal = crate::SourceAstFactory::expression(crate::ExprKind::Name(name), range)
                .expect("parsed input");
            let binary = |op, left, right| {
                crate::SourceAstFactory::expression(
                    crate::ExprKind::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    TextRange::new(range.start(), end),
                )
                .expect("parsed interval")
            };
            let low = binary(crate::BinaryOp::GreaterEqual, formal.clone(), lower);
            let high = binary(crate::BinaryOp::LessEqual, formal, upper);
            Some(Some(binary(crate::BinaryOp::And, low, high)))
        } else {
            self.parse_expression(0).map(Some)
        }
    }

    fn parse_property_table(&mut self) -> Option<crate::PropertySourceSyntax> {
        let start = self.expect_keyword("table")?.range().start();
        self.expect(TokenKind::LeftBrace, "`{` after table")?;
        self.expect_keyword("data")?;
        let data = self.parse_name_path("exact table asset")?;
        self.expect(TokenKind::Semicolon, "`;` after table asset")?;
        self.expect_keyword("axis")?;
        let axis = self.declaration_name("table input")?.text().to_owned();
        self.expect(TokenKind::Colon, "`:` after table input")?;
        let axis_dimension = self.parse_dimension_expression()?;
        self.expect(TokenKind::Semicolon, "`;` after table axis")?;
        self.expect_keyword("value")?;
        let value = self
            .declaration_name("table result column")?
            .text()
            .to_owned();
        self.expect(TokenKind::Colon, "`:` after result column")?;
        let value_dimension = self.parse_dimension_expression()?;
        self.expect(TokenKind::Semicolon, "`;` after result column")?;
        for (key, policy) in [
            ("interpolation", "piecewise_affine"),
            ("preprocessing", "identity"),
            ("missing", "reject"),
            ("knot_derivative", "reject"),
            ("endpoint_derivative", "reject"),
        ] {
            self.expect_keyword(key)?;
            self.expect_keyword(policy)?;
            self.expect(TokenKind::Semicolon, "`;` after exact table policy")?;
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after table")?
            .range()
            .end();
        self.expect_keyword("validity")?;
        let selected = self.declaration_name("validity input")?.text().to_owned();
        if selected != axis {
            self.error_here("validity must name the exact table input");
            return None;
        }
        self.expect_keyword("in")?;
        self.expect(TokenKind::LeftBracket, "`[` before table validity")?;
        let lower = self.parse_expression(0)?;
        self.expect(TokenKind::Comma, "`,` between validity endpoints")?;
        let upper = self.parse_expression(0)?;
        self.expect(TokenKind::RightBracket, "`]` after table validity")?;
        self.expect(TokenKind::Semicolon, "`;` after table validity")?;
        Some(crate::PropertySourceSyntax::Table(Box::new(
            crate::PropertyTableSyntax {
                data,
                axis,
                axis_dimension,
                value,
                value_dimension,
                validity: [lower, upper],
                range: TextRange::new(start, end),
            },
        )))
    }

    pub(super) fn at_component_property(&mut self) -> bool {
        self.at_keyword("public")
            && self.following_significant_token().is_some_and(|token| {
                token.kind() == TokenKind::Identifier && token.text() == "property"
            })
    }

    pub(super) fn parse_property_binding(&mut self, start: u32) -> Option<PropertyBindingDecl> {
        self.expect_keyword("property")?;
        let property = self
            .expect_identifier("public property requirement name")?
            .text()
            .to_owned();
        self.expect(TokenKind::Equal, "`=` in property binding")?;
        let release = self.parse_name_path("property release name")?;
        let end = release.range().end();
        Some(PropertyBindingDecl {
            comments: Default::default(),
            property,
            release,
            range: TextRange::new(start, end),
        })
    }

    pub(super) fn parse_material_composition(
        &mut self,
        start: u32,
        visibility: VisibilitySyntax,
    ) -> Option<MaterialCompositionDecl> {
        self.expect_keyword("material")?;
        self.expect_keyword("composition")?;
        let name = self
            .declaration_name("material composition name")?
            .text()
            .to_owned();
        self.expect(TokenKind::LeftBrace, "`{` after material composition name")?;
        let mut properties = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let binding_start = self.current().range().start();
            let mut binding = self.parse_property_binding(binding_start)?;
            binding.range = TextRange::new(
                binding.range.start(),
                self.expect(TokenKind::Semicolon, "`;` after material property binding")?
                    .range()
                    .end(),
            );
            properties.push(binding);
        }
        let end = self
            .expect(TokenKind::RightBrace, "`}` after material composition")?
            .range()
            .end();
        Some(MaterialCompositionDecl {
            comments: Default::default(),
            visibility,
            name,
            properties,
            range: TextRange::new(start, end),
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn analytic_signature_validity_and_branch_round_trip() {
        let source = r#"property contract Conductivity(input T: K, input pressure: kg / m / s ^ 2): kg * m / s ^ 3 / K {
  derivatives first_partials;
  branch liquid;
}
property release Fluid: Conductivity {
  analytic { value = 10[kg * m / s ^ 3 / K] + 0.1[kg * m / s ^ 3 / K ^ 2] * (T - 273[K]);
  source_unit: kg * m / s ^ 3 / K = 1; }
  validity T >= 273[K] and T <= 373[K];
  outside reject;
  branch liquid;
  citation org.example.independent;
  license spdx.CC0_1_0;
}
"#;
        let parsed = crate::parse("analytic.eqi", source)
            .into_document()
            .expect("analytic syntax");
        let (_, inputs, partials, branch) = parsed.property_contract_profiles().next().unwrap();
        assert_eq!(
            inputs
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["T", "pressure"]
        );
        assert_eq!(
            partials,
            eqiora_schema::kernel::PropertyDerivatives::FirstPartials
        );
        assert_eq!(branch.unwrap().as_str(), "liquid");
        assert!(
            parsed
                .property_release_profiles()
                .next()
                .unwrap()
                .1
                .is_some()
        );
        let formatted = crate::format(&parsed);
        let reopened = crate::parse("formatted.eqi", &formatted)
            .into_document()
            .expect("formatted analytic syntax");
        assert_eq!(crate::format(&reopened), formatted);
        assert!(
            crate::parse(
                "bad.eqi",
                &source.replace("outside reject;", "outside extrapolate;")
            )
            .into_document()
            .is_err()
        );
    }

    #[test]
    fn contract_release_requirement_and_binding_round_trip() {
        let source = r#"public property contract Diffusivity(): m ^ 2 / s {
  derivatives value_only;
}

property release ReferenceDiffusivity: Diffusivity {
  analytic { value = 25;
  source_unit: m ^ 2 / s = 1 / 1000; }
  validity unconditional; outside reject; branch single;
  citation org.example.measurement;
  license spdx.CC0_1_0;
}

public material composition ReferenceMaterial {
  property diffusivity = ReferenceDiffusivity;
}

public component Diffusion(property diffusivity: Diffusivity) {
  relation law { diffusivity = 0; }
}

model Main() {
  instance domain: Diffusion(diffusivity = ReferenceMaterial.diffusivity);
}"#;
        let document = crate::parse("property.eqi", source)
            .into_document()
            .expect("valid property source");
        assert_eq!(document.property_contract_syntax().len(), 1);
        assert_eq!(document.property_release_syntax().len(), 1);
        assert_eq!(document.material_composition_syntax().len(), 1);
        assert_eq!(
            document.components()[0]
                .signature()
                .iter()
                .filter(|item| matches!(item, crate::SignatureItem::Property(_)))
                .count(),
            1
        );
        let formatted = crate::format(&document);
        let reparsed = crate::parse("formatted.eqi", &formatted)
            .into_document()
            .expect("formatted source reparses");
        assert_eq!(crate::format(&reparsed), formatted);
    }
}
