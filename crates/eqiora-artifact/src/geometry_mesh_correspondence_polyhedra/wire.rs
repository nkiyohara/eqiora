use super::*;

impl WirePolyhedra {
    pub(in crate::geometry_mesh_correspondence) fn validate_local(
        &self,
        limits: GeometryDecoderLimits,
    ) -> Result<(), Diagnostic> {
        if self.schema != CORRESPONDENCE_SCHEMA
            || self.encoding != CANONICAL_ENCODING
            || self.source != SOURCE
            || self.dimension != 3
        {
            return Err(invalid_artifact(
                "unsupported polyhedral correspondence contract",
            ));
        }
        ArtifactDigest::from_hex(self.geometry_sha256.clone())?;
        ArtifactDigest::from_hex(self.mesh_sha256.clone())?;
        let rows = self
            .vertices
            .len()
            .checked_add(self.edges.len())
            .and_then(|n| n.checked_add(self.volumes.len()))
            .and_then(|n| n.checked_add(self.frontiers.len()))
            .ok_or_else(|| invalid_artifact("polyhedral assignment count overflows"))?;
        if rows > limits.max_geometry_entities
            || self.vertices.is_empty()
            || self.edges.is_empty()
            || self.volumes.is_empty()
            || self.frontiers.is_empty()
        {
            return Err(invalid_artifact(
                "polyhedral assignment count is empty or exceeds limits",
            ));
        }
        let mut memberships = 0usize;
        for (dimension, rows) in [(0, &self.vertices), (1, &self.edges), (3, &self.volumes)] {
            let mut seen = BTreeSet::new();
            for (id, row) in rows.iter().enumerate() {
                if row.geometry_entity != id as u64
                    || row.mesh_entities.is_empty()
                    || !row.mesh_entities.windows(2).all(|w| w[0] < w[1])
                    || dimension == 0 && row.mesh_entities.len() != 1
                {
                    return Err(invalid_artifact(
                        "polyhedral assignments must be complete, nonempty, and canonical",
                    ));
                }
                for &member in &row.mesh_entities {
                    usize::try_from(member).map_err(|_| {
                        invalid_artifact("polyhedral mesh index exceeds local usize")
                    })?;
                    if !seen.insert(member) {
                        return Err(invalid_artifact(
                            "polyhedral mesh membership is duplicate or ambiguous",
                        ));
                    }
                }
                memberships = memberships
                    .checked_add(row.mesh_entities.len())
                    .ok_or_else(|| invalid_artifact("polyhedral membership count overflows"))?;
            }
        }
        if !self.frontiers.windows(2).all(|w| {
            (w[0].parent_volume, w[0].geometry_facet) < (w[1].parent_volume, w[1].geometry_facet)
        }) {
            return Err(invalid_artifact(
                "polyhedral frontier rows are duplicate or noncanonical",
            ));
        }
        let mut seen = BTreeSet::new();
        for row in &self.frontiers {
            if row.parent_volume >= self.volumes.len() as u64
                || row.facet_indices.is_empty()
                || row.facet_indices.len() != row.parent_outward.len()
                || !row.facet_indices.windows(2).all(|w| w[0] < w[1])
            {
                return Err(invalid_artifact(
                    "polyhedral frontiers require canonical nonempty outward incidence",
                ));
            }
            usize::try_from(row.geometry_facet)
                .map_err(|_| invalid_artifact("polyhedral facet exceeds local usize"))?;
            for &id in &row.facet_indices {
                usize::try_from(id)
                    .map_err(|_| invalid_artifact("polyhedral mesh facet exceeds local usize"))?;
                if !seen.insert((row.parent_volume, id)) {
                    return Err(invalid_artifact(
                        "polyhedral parent frontier membership is duplicate",
                    ));
                }
            }
            memberships = memberships
                .checked_add(row.facet_indices.len())
                .ok_or_else(|| invalid_artifact("polyhedral membership count overflows"))?;
        }
        if memberships > limits.max_geometry_mesh_memberships {
            return Err(invalid_artifact(
                "polyhedral memberships exceed decoder limit",
            ));
        }
        Ok(())
    }
}
