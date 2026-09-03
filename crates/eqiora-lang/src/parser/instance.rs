use super::*;

impl Parser<'_> {
    pub(super) fn parse_instance(&mut self) -> Option<InstanceDecl> {
        let start = self.expect_keyword("instance")?.range().start();
        let name = self.expect_identifier("instance name")?.text().to_owned();
        self.expect(TokenKind::Colon, "`:` before component definition")?;
        let definition = self.parse_name_path("component definition name")?;
        let mut bindings = Vec::new();
        let mut support_bindings = Vec::new();
        let mut boundary_set_bindings = Vec::new();
        let mut field_bindings = Vec::new();
        let mut property_bindings = Vec::new();
        let mut material_binding = None;
        if self.at(TokenKind::LeftParen) {
            self.bump();
            if !self.at(TokenKind::RightParen) {
                loop {
                    let binding_start = self.current().range().start();
                    if self.at_support_binding() {
                        self.bump();
                        let slot = self
                            .expect_identifier("public support-slot binding name")?
                            .text()
                            .to_owned();
                        self.expect(TokenKind::Equal, "`=` in support binding")?;
                        if self.at_keyword("boundaries") {
                            self.bump();
                            self.expect(TokenKind::LeftParen, "`(` after `boundaries`")?;
                            let mut members = Vec::new();
                            if !self.at(TokenKind::RightParen) {
                                loop {
                                    let member =
                                        self.expect_identifier("boundary Domain member")?;
                                    members.push(BoundarySetMemberSyntax {
                                        target: member.text().to_owned(),
                                        range: member.range(),
                                    });
                                    if !self.at(TokenKind::Comma) {
                                        break;
                                    }
                                    self.bump();
                                }
                            }
                            let close =
                                self.expect(TokenKind::RightParen, "`)` after boundary members")?;
                            boundary_set_bindings.push(BoundarySetBindingDecl {
                                slot,
                                members,
                                range: TextRange::new(binding_start, close.range().end()),
                            });
                        } else {
                            let target =
                                self.expect_identifier("enclosing Domain or support-slot name")?;
                            support_bindings.push(SupportBindingDecl {
                                slot,
                                target: target.text().to_owned(),
                                range: TextRange::new(binding_start, target.range().end()),
                            });
                        }
                        if !self.at(TokenKind::Comma) {
                            break;
                        }
                        self.bump();
                        continue;
                    }
                    if self.at_field_binding() {
                        self.bump();
                        let slot = self
                            .expect_identifier("public Field-slot binding name")?
                            .text()
                            .to_owned();
                        self.expect(TokenKind::Equal, "`=` in Field binding")?;
                        let target =
                            self.expect_identifier("enclosing Field or Field-slot name")?;
                        field_bindings.push(FieldBindingDecl {
                            slot,
                            target: target.text().to_owned(),
                            range: TextRange::new(binding_start, target.range().end()),
                        });
                        if !self.at(TokenKind::Comma) {
                            break;
                        }
                        self.bump();
                        continue;
                    }
                    if self.at_keyword("property") {
                        property_bindings.push(self.parse_property_binding(binding_start)?);
                        if !self.at(TokenKind::Comma) {
                            break;
                        }
                        self.bump();
                        continue;
                    }
                    if self.at_keyword("material") {
                        self.bump();
                        self.expect(TokenKind::Equal, "`=` in material composition binding")?;
                        let composition = self.parse_name_path("material composition name")?;
                        if material_binding.replace(composition).is_some() {
                            self.error_here("an instance has at most one material composition");
                        }
                        if !self.at(TokenKind::Comma) {
                            break;
                        }
                        self.bump();
                        continue;
                    }
                    let parameter = self
                        .expect_identifier("public Parameter binding name")?
                        .text()
                        .to_owned();
                    self.expect(TokenKind::Equal, "`=` in Parameter binding")?;
                    let value = self.parse_expression(0)?;
                    bindings.push(ParameterBindingDecl {
                        parameter,
                        range: TextRange::new(binding_start, value.range().end()),
                        value,
                    });
                    if !self.at(TokenKind::Comma) {
                        break;
                    }
                    self.bump();
                }
            }
            self.expect(TokenKind::RightParen, "`)` after Parameter bindings")?;
        }
        let end = self
            .expect(TokenKind::Semicolon, "`;` after instance")?
            .range()
            .end();
        Some(InstanceDecl {
            name,
            definition,
            bindings,
            support_bindings,
            boundary_set_bindings,
            field_bindings,
            property_bindings,
            material_binding,
            range: TextRange::new(start, end),
        })
    }
}
