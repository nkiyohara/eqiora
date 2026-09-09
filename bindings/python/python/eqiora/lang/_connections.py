"""Immutable named physical ports; the compiler owns connection semantics."""

from . import (
    Expression, ModuleError, ValueType, _Ast, _CREATE, _MAX_DECLARATIONS,
    _doc, _name,
)


class Connector:
    """A nominal scalar across/through declaration in one Module.

    Authority: ``bindings/python/python/eqiora/lang/_connections.py::Connector``.
    """

    __slots__ = ("_owner", "_name", "_across", "_through", "_doc")

    def __init__(self, token, owner, name, across, through, doc):
        if token is not _CREATE:
            raise TypeError("connectors are created by Module.connector()")
        for key, value in (("_owner", owner), ("_name", name), ("_across", across),
                           ("_through", through), ("_doc", doc)):
            object.__setattr__(self, key, value)

    def __setattr__(self, name, value):
        raise AttributeError("Connector is immutable")

    def __repr__(self):
        return f"Connector({self._name!r})"


class Port:
    """An exact physical endpoint; declared quantities are symbolic expressions.

    Use member(name) when a declared name overlaps a Python attribute.

    Authority: ``bindings/python/python/eqiora/lang/_connections.py::Port``.
    """

    __slots__ = ("_owner", "_name", "_connector")

    def __init__(self, token, owner, name, connector):
        if token is not _CREATE:
            raise TypeError("ports are created by Component.port() or instance()")
        object.__setattr__(self, "_owner", owner)
        object.__setattr__(self, "_name", name)
        object.__setattr__(self, "_connector", connector)

    def __getattr__(self, name):
        return self.member(name)

    def member(self, name: str) -> Expression:
        """Select an exact declared quantity, including Python attribute names.

        Authority: ``bindings/python/python/eqiora/lang/_connections.py::Port.member``.
        """
        if name not in (self._connector._across[0], self._connector._through[0]):
            raise AttributeError(f"Connector {self._connector._name!r} has no quantity {name!r}")
        return Expression(_CREATE, _Ast.name(f"{self._name}.{name}"), self._owner)

    def __setattr__(self, name, value):
        raise AttributeError("Port is immutable")

    def __repr__(self):
        return f"Port({self._name!r}, connector={self._connector._name!r})"


def connector(module, name, *, across, through, doc=None):
    def quantity(value):
        if not isinstance(value, tuple) or len(value) != 2:
            raise TypeError("connector quantity must be a (name, ValueType) pair")
        member, kind = value
        member = _name(member)
        if not isinstance(kind, ValueType):
            raise TypeError("connector quantity needs an eqiora.ValueType")
        return member, module._type_syntax(kind)

    across, through = quantity(across), quantity(through)
    if across[0] == through[0]:
        raise ModuleError("connector quantity names must be distinct")
    documentation = _doc(doc)
    admitted = module._add_top_name(name)
    value = Connector(_CREATE, module._owner, admitted, across, through, documentation)
    module._connectors.append(value)
    return value


def port(component, name, *, connector, doc=None):
    if not isinstance(connector, Connector) or connector._owner is not component._owner:
        raise ModuleError("port connector must belong to this Module")
    documentation = _doc(doc)
    admitted = component._add_name(name)
    value = Port(_CREATE, component._component_token, admitted, connector)
    component._ports.append((value, documentation))
    return value


def connect(component, *ports, doc=None):
    component._source._ensure_open()
    if not 2 <= len(ports) <= _MAX_DECLARATIONS:
        raise ModuleError("a connection needs between 2 and 256 physical ports")
    for port in ports:
        if not isinstance(port, Port) or port._owner is not component._component_token:
            raise ModuleError("connection endpoints must be physical ports of this Component")
    documentation = _doc(doc)
    if component._declaration_count >= _MAX_DECLARATIONS:
        raise ModuleError("Component exceeds the 256-declaration limit")
    component._declaration_count += 1
    component._connections.append((ports, documentation))


def instance_ports(component, target, name):
    """Expose authored local physical endpoints with their occurrence identity."""
    return {
        port._name: Port(_CREATE, component._component_token,
                         f"{name}.{port._name}", port._connector)
        for port, _ in target._ports
    }
