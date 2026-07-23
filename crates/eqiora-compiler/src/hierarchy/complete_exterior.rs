//! Identity-parametric proof of one complete Cartesian exterior.
//!
//! This module is deliberately independent of source syntax and the Semantic
//! Kernel. Occurrence elaboration supplies exact Domain identities and a
//! resolver; a successful proof returns only a canonical side-to-identity
//! bijection. Occurrence binding and occurrence-free definition checking both
//! use this same proof; neither is permitted to infer missing sides.

use eqiora_schema::kernel::BoundarySide;

/// One resolved Cartesian Domain needed by the complete-exterior proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CartesianDomain<I> {
    /// Exact Cartesian volume identity and its authoritative dimension.
    Volume { ambient_dimension: usize },
    /// Exact Cartesian boundary identity metadata.
    Boundary {
        exact_parent: I,
        ambient_dimension: usize,
        axis: usize,
        side: BoundarySide,
    },
}

/// An explicit, nonempty set of sorted unique exact Boundary identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BoundarySet<I> {
    exact_parent: I,
    members: Box<[I]>,
}

impl<I> BoundarySet<I>
where
    I: Clone + Ord,
{
    pub(super) fn try_new(
        exact_parent: I,
        members: impl IntoIterator<Item = I>,
    ) -> Result<Self, CompleteExteriorError<I>> {
        let mut canonical_members = Vec::new();
        for member in members {
            if canonical_members.len() == canonical_members.capacity() {
                let count = canonical_members.len().checked_add(1).ok_or(
                    CompleteExteriorError::Allocation {
                        resource: "exact Boundary identities",
                        count: usize::MAX,
                    },
                )?;
                canonical_members.try_reserve(1).map_err(|_| {
                    CompleteExteriorError::Allocation {
                        resource: "exact Boundary identities",
                        count,
                    }
                })?;
            }
            canonical_members.push(member);
        }
        let mut members = canonical_members;
        if members.is_empty() {
            return Err(CompleteExteriorError::Empty);
        }
        members.sort_unstable();
        if let Some(pair) = members.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(CompleteExteriorError::DuplicateExactMember {
                member: pair[0].clone(),
            });
        }
        Ok(Self {
            exact_parent,
            members: members.into_boxed_slice(),
        })
    }

    /// Exact parent supplied by the occurrence's volume-support binding.
    #[cfg(test)]
    pub(super) const fn exact_parent(&self) -> &I {
        &self.exact_parent
    }

    /// Members in canonical exact-identity order, never author list order.
    #[cfg(test)]
    pub(super) const fn members(&self) -> &[I] {
        &self.members
    }
}

/// One canonical Cartesian side and its unique exact Boundary identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompleteExteriorSide<I> {
    axis: usize,
    side: BoundarySide,
    boundary: I,
}

impl<I> CompleteExteriorSide<I> {
    pub(super) const fn axis(&self) -> usize {
        self.axis
    }

    pub(super) const fn side(&self) -> BoundarySide {
        self.side
    }

    pub(super) const fn boundary(&self) -> &I {
        &self.boundary
    }
}

/// Successful bijection from every Cartesian side to one exact Boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompleteExteriorWitness<I> {
    exact_parent: I,
    ambient_dimension: usize,
    sides: Box<[CompleteExteriorSide<I>]>,
}

impl<I> CompleteExteriorWitness<I> {
    pub(super) const fn exact_parent(&self) -> &I {
        &self.exact_parent
    }

    pub(super) const fn ambient_dimension(&self) -> usize {
        self.ambient_dimension
    }

    pub(super) const fn sides(&self) -> &[CompleteExteriorSide<I>] {
        &self.sides
    }
}

/// Closed failures of the compiler-local Cartesian completeness proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CompleteExteriorError<I> {
    Empty,
    DuplicateExactMember {
        member: I,
    },
    UnknownDomain {
        domain: I,
    },
    ExactParentIsNotVolume {
        exact_parent: I,
    },
    ExactParentHasZeroDimension {
        exact_parent: I,
    },
    MemberIsVolume {
        member: I,
    },
    BoundaryOfBoundary {
        member: I,
        actual_parent: I,
    },
    WrongParent {
        member: I,
        expected_parent: I,
        actual_parent: I,
    },
    WrongDimension {
        member: I,
        expected: usize,
        actual: usize,
    },
    AxisOutsideParentDimension {
        member: I,
        axis: usize,
        ambient_dimension: usize,
    },
    DuplicateGeometry {
        axis: usize,
        side: BoundarySide,
        first: I,
        duplicate: I,
    },
    MissingSide {
        axis: usize,
        side: BoundarySide,
    },
    SideCountOverflow {
        ambient_dimension: usize,
    },
    Allocation {
        resource: &'static str,
        count: usize,
    },
}

/// Prove an explicit Boundary set is exactly the exterior of one Cartesian
/// volume.
///
/// The parent dimension is read once from `resolve(exact_parent)`. Boundary
/// declaration order and lexical side names never participate. Validation is
/// atomic: no partial witness is returned for any missing, repeated, or
/// ill-typed member.
pub(super) fn prove_complete_cartesian_exterior<I>(
    exact_parent: I,
    members: impl IntoIterator<Item = I>,
    mut resolve: impl FnMut(&I) -> Option<CartesianDomain<I>>,
) -> Result<CompleteExteriorWitness<I>, CompleteExteriorError<I>>
where
    I: Clone + Ord,
{
    let set = BoundarySet::try_new(exact_parent, members)?;
    prove_boundary_set(set, &mut resolve)
}

fn prove_boundary_set<I>(
    set: BoundarySet<I>,
    resolve: &mut impl FnMut(&I) -> Option<CartesianDomain<I>>,
) -> Result<CompleteExteriorWitness<I>, CompleteExteriorError<I>>
where
    I: Clone + Ord,
{
    let parent =
        resolve(&set.exact_parent).ok_or_else(|| CompleteExteriorError::UnknownDomain {
            domain: set.exact_parent.clone(),
        })?;
    let CartesianDomain::Volume { ambient_dimension } = parent else {
        return Err(CompleteExteriorError::ExactParentIsNotVolume {
            exact_parent: set.exact_parent,
        });
    };
    if ambient_dimension == 0 {
        return Err(CompleteExteriorError::ExactParentHasZeroDimension {
            exact_parent: set.exact_parent,
        });
    }
    let side_count = ambient_dimension
        .checked_mul(2)
        .ok_or(CompleteExteriorError::SideCountOverflow { ambient_dimension })?;
    let mut sides = Vec::new();
    sides
        .try_reserve_exact(side_count)
        .map_err(|_| CompleteExteriorError::Allocation {
            resource: "Cartesian exterior side slots",
            count: side_count,
        })?;
    sides.resize_with(side_count, || None);

    for member in &set.members {
        let domain = resolve(member).ok_or_else(|| CompleteExteriorError::UnknownDomain {
            domain: member.clone(),
        })?;
        let CartesianDomain::Boundary {
            exact_parent: actual_parent,
            ambient_dimension: actual_dimension,
            axis,
            side,
        } = domain
        else {
            return Err(CompleteExteriorError::MemberIsVolume {
                member: member.clone(),
            });
        };

        if actual_parent != set.exact_parent {
            match resolve(&actual_parent) {
                Some(CartesianDomain::Boundary { .. }) => {
                    return Err(CompleteExteriorError::BoundaryOfBoundary {
                        member: member.clone(),
                        actual_parent,
                    });
                }
                Some(CartesianDomain::Volume { .. }) => {
                    return Err(CompleteExteriorError::WrongParent {
                        member: member.clone(),
                        expected_parent: set.exact_parent.clone(),
                        actual_parent,
                    });
                }
                None => {
                    return Err(CompleteExteriorError::UnknownDomain {
                        domain: actual_parent,
                    });
                }
            }
        }
        if actual_dimension != ambient_dimension {
            return Err(CompleteExteriorError::WrongDimension {
                member: member.clone(),
                expected: ambient_dimension,
                actual: actual_dimension,
            });
        }
        if axis >= ambient_dimension {
            return Err(CompleteExteriorError::AxisOutsideParentDimension {
                member: member.clone(),
                axis,
                ambient_dimension,
            });
        }
        let side_offset = match side {
            BoundarySide::Lower => 0,
            BoundarySide::Upper => 1,
        };
        let index = axis
            .checked_mul(2)
            .and_then(|value| value.checked_add(side_offset))
            .ok_or(CompleteExteriorError::SideCountOverflow { ambient_dimension })?;
        if let Some(first) = sides[index].replace(member.clone()) {
            return Err(CompleteExteriorError::DuplicateGeometry {
                axis,
                side,
                first,
                duplicate: member.clone(),
            });
        }
    }

    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(side_count)
        .map_err(|_| CompleteExteriorError::Allocation {
            resource: "complete-exterior witness sides",
            count: side_count,
        })?;
    for axis in 0..ambient_dimension {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            let side_offset = usize::from(matches!(side, BoundarySide::Upper));
            let index = axis * 2 + side_offset;
            let boundary = sides[index]
                .take()
                .ok_or(CompleteExteriorError::MissingSide { axis, side })?;
            canonical.push(CompleteExteriorSide {
                axis,
                side,
                boundary,
            });
        }
    }

    Ok(CompleteExteriorWitness {
        exact_parent: set.exact_parent,
        ambient_dimension,
        sides: canonical.into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    type Identity = &'static str;

    fn volume(ambient_dimension: usize) -> CartesianDomain<Identity> {
        CartesianDomain::Volume { ambient_dimension }
    }

    fn boundary(
        exact_parent: Identity,
        ambient_dimension: usize,
        axis: usize,
        side: BoundarySide,
    ) -> CartesianDomain<Identity> {
        CartesianDomain::Boundary {
            exact_parent,
            ambient_dimension,
            axis,
            side,
        }
    }

    fn square() -> BTreeMap<Identity, CartesianDomain<Identity>> {
        BTreeMap::from([
            ("body", volume(2)),
            ("x_lower", boundary("body", 2, 0, BoundarySide::Lower)),
            ("x_upper", boundary("body", 2, 0, BoundarySide::Upper)),
            ("y_lower", boundary("body", 2, 1, BoundarySide::Lower)),
            ("y_upper", boundary("body", 2, 1, BoundarySide::Upper)),
        ])
    }

    fn prove(
        domains: &BTreeMap<Identity, CartesianDomain<Identity>>,
        members: impl IntoIterator<Item = Identity>,
    ) -> Result<CompleteExteriorWitness<Identity>, CompleteExteriorError<Identity>> {
        prove_complete_cartesian_exterior("body", members, |identity| {
            domains.get(identity).cloned()
        })
    }

    #[test]
    fn derives_dimension_and_canonical_bijection_from_exact_parent() {
        let mut domains = square();
        domains.insert(
            "unselected_duplicate",
            boundary("body", 2, 0, BoundarySide::Lower),
        );
        let witness = prove(&domains, ["y_upper", "x_lower", "y_lower", "x_upper"])
            .expect("unordered complete exterior is valid");

        assert_eq!(witness.exact_parent(), &"body");
        assert_eq!(witness.ambient_dimension(), 2);
        assert_eq!(
            witness
                .sides()
                .iter()
                .map(|side| (side.axis(), side.side(), *side.boundary()))
                .collect::<Vec<_>>(),
            [
                (0, BoundarySide::Lower, "x_lower"),
                (0, BoundarySide::Upper, "x_upper"),
                (1, BoundarySide::Lower, "y_lower"),
                (1, BoundarySide::Upper, "y_upper"),
            ]
        );

        let set = BoundarySet::try_new("body", ["y_upper", "x_lower", "y_lower", "x_upper"])
            .expect("complete input has a valid exact set");
        assert_eq!(set.exact_parent(), &"body");
        assert_eq!(set.members(), ["x_lower", "x_upper", "y_lower", "y_upper"]);
    }

    #[test]
    fn empty_and_duplicate_exact_members_fail_before_geometry() {
        let domains = square();
        assert_eq!(prove(&domains, []), Err(CompleteExteriorError::Empty));
        assert_eq!(
            prove(&domains, ["x_lower", "x_upper", "y_lower", "x_lower"]),
            Err(CompleteExteriorError::DuplicateExactMember { member: "x_lower" })
        );
    }

    #[test]
    fn missing_side_and_distinct_duplicate_geometry_are_rejected() {
        let mut domains = square();
        domains.insert(
            "another_x_lower",
            boundary("body", 2, 0, BoundarySide::Lower),
        );
        assert_eq!(
            prove(&domains, ["x_lower", "x_upper", "y_lower"]),
            Err(CompleteExteriorError::MissingSide {
                axis: 1,
                side: BoundarySide::Upper,
            })
        );
        assert_eq!(
            prove(
                &domains,
                [
                    "x_lower",
                    "another_x_lower",
                    "x_upper",
                    "y_lower",
                    "y_upper",
                ]
            ),
            Err(CompleteExteriorError::DuplicateGeometry {
                axis: 0,
                side: BoundarySide::Lower,
                first: "another_x_lower",
                duplicate: "x_lower",
            })
        );
    }

    #[test]
    fn wrong_parent_volume_member_and_boundary_of_boundary_are_distinct() {
        let mut domains = square();
        domains.insert("other", volume(2));
        domains.insert("other_side", boundary("other", 2, 1, BoundarySide::Upper));
        domains.insert("edge", boundary("x_lower", 2, 1, BoundarySide::Lower));

        assert_eq!(
            prove(&domains, ["x_lower", "x_upper", "y_lower", "other_side"]),
            Err(CompleteExteriorError::WrongParent {
                member: "other_side",
                expected_parent: "body",
                actual_parent: "other",
            })
        );
        assert_eq!(
            prove(&domains, ["body", "x_upper", "y_lower", "y_upper"]),
            Err(CompleteExteriorError::MemberIsVolume { member: "body" })
        );
        assert_eq!(
            prove(&domains, ["x_lower", "x_upper", "y_upper", "edge"]),
            Err(CompleteExteriorError::BoundaryOfBoundary {
                member: "edge",
                actual_parent: "x_lower",
            })
        );
    }

    #[test]
    fn wrong_dimension_and_invalid_axis_fail_closed() {
        let mut domains = square();
        domains.insert(
            "wrong_dimension",
            boundary("body", 3, 1, BoundarySide::Upper),
        );
        domains.insert("wrong_axis", boundary("body", 2, 2, BoundarySide::Upper));
        assert_eq!(
            prove(
                &domains,
                ["x_lower", "x_upper", "y_lower", "wrong_dimension"]
            ),
            Err(CompleteExteriorError::WrongDimension {
                member: "wrong_dimension",
                expected: 2,
                actual: 3,
            })
        );
        assert_eq!(
            prove(&domains, ["x_lower", "x_upper", "y_lower", "wrong_axis"]),
            Err(CompleteExteriorError::AxisOutsideParentDimension {
                member: "wrong_axis",
                axis: 2,
                ambient_dimension: 2,
            })
        );
    }

    #[test]
    fn exact_parent_shape_and_side_count_arithmetic_fail_closed() {
        let boundary_parent = BTreeMap::from([
            ("body", boundary("outer", 2, 0, BoundarySide::Lower)),
            ("outer", volume(2)),
        ]);
        assert_eq!(
            prove_complete_cartesian_exterior("body", ["body"], |identity| {
                boundary_parent.get(identity).cloned()
            }),
            Err(CompleteExteriorError::ExactParentIsNotVolume {
                exact_parent: "body",
            })
        );

        let zero = BTreeMap::from([("body", volume(0))]);
        assert_eq!(
            prove(&zero, ["member"]),
            Err(CompleteExteriorError::ExactParentHasZeroDimension {
                exact_parent: "body",
            })
        );

        let overflow = BTreeMap::from([("body", volume(usize::MAX))]);
        assert_eq!(
            prove(&overflow, ["member"]),
            Err(CompleteExteriorError::SideCountOverflow {
                ambient_dimension: usize::MAX,
            })
        );
    }
}
