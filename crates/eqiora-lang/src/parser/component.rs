//! Private Component body declaration dispatch.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_component_item(&mut self) -> Option<ParsedComponentItem> {
        let public = self.at_keyword("public");
        let start = if public {
            self.bump().range().start()
        } else {
            self.current().range().start()
        };
        let visibility = if public {
            VisibilitySyntax::Public
        } else {
            VisibilitySyntax::Private
        };

        if self.at_keyword("parameter") {
            return self
                .parse_component_parameter(start, visibility)
                .map(ComponentItem::Parameter)
                .map(Box::new)
                .map(ParsedComponentItem::Retained);
        }
        if self.at_keyword("port") {
            return self.parse_component_port(start, visibility).map(|port| {
                ParsedComponentItem::Retained(Box::new(match port {
                    ParsedComponentPort::Ordinary(port) => ComponentItem::Port(port),
                    ParsedComponentPort::Family(port) => ComponentItem::PortFamily(port),
                }))
            });
        }
        if public {
            let token = self.current().clone();
            self.error_token(
                &token,
                "only `parameter` and `port` body declarations may be public; support and unknown requirements belong in the signature",
            );
            if self.at_keyword("variable") || self.at_keyword("state") {
                self.parse_field(true)?;
            } else if self.at_keyword("clock") {
                self.parse_clock()?;
            } else if self.at_keyword("relation") {
                self.parse_component_relation()?;
            } else if self.at_keyword("connect") {
                self.parse_connection(true)?;
            } else if self.at_keyword("instance") {
                self.parse_instance()?;
            } else {
                return None;
            }
            return Some(ParsedComponentItem::Discarded);
        }

        let item = if self.at_keyword("let") {
            self.parse_let().map(ComponentItem::Let)
        } else if self.at_keyword("variable") || self.at_keyword("state") {
            self.parse_field(true).map(ComponentItem::Field)
        } else if self.at_keyword("initial") {
            self.parse_initial().map(ComponentItem::Initial)
        } else if self.at_keyword("clock") {
            self.parse_clock().map(ComponentItem::Clock)
        } else if self.at_keyword("relation") {
            self.parse_component_relation()
                .map(|relation| match relation {
                    ParsedRelation::Ordinary(relation) => ComponentItem::Relation(relation),
                    ParsedRelation::Family(relation) => ComponentItem::RelationFamily(relation),
                })
        } else if self.at_keyword("connect") {
            self.parse_connection(true)
                .map(|connection| match connection {
                    ParsedConnection::Ordinary(connection) => ComponentItem::Connection(connection),
                    ParsedConnection::Boundary(connection) => {
                        ComponentItem::BoundaryConnection(connection)
                    }
                })
        } else if self.at_keyword("instance") {
            self.parse_instance().map(ComponentItem::Instance)
        } else {
            self.error_here(
                "expected parameter, port, variable, state, initial, clock, relation, connect, or instance in component",
            );
            None
        }?;
        Some(ParsedComponentItem::Retained(Box::new(item)))
    }
}
