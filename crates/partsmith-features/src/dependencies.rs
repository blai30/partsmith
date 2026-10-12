//! What a feature declares it reads and defines (features.md section 2.4).

use partsmith_model::{Expr, FieldUnit, Id};

use crate::params::{AxisRef, PlaneRef};

/// Everything a feature reads, declared before it builds. The context refuses undeclared reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dependencies {
    /// Features whose output is read directly (sketches, datums), in params order, without duplicate ids.
    pub features: Vec<Link>,
    /// Bodies whose state is read but not written, in params order, without duplicate ids.
    pub bodies_read: Vec<Link>,
    /// Bodies this feature may replace. Their prior state is also readable. No id appears in both lists.
    pub bodies_written: Vec<Link>,
    /// Every expression stored in the params, with its field path and unit, in params order. `build` may leave some unevaluated (sketch reference dimensions).
    pub expressions: Vec<ExprUse>,
    /// Ids the params define (sketch entity and constraint ids), in params order. Empty for most features.
    pub declared_ids: Vec<Id>,
    /// Asset files read, by name (a content hash such as "<64 hex>.step"), in params order, without duplicates.
    pub assets: Vec<AssetLink>,
}

/// A feature or body a feature refers to, with the field that names it (for messages and highlighting).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// The feature or body id.
    pub id: Id,
    /// Path of the first params field that names it, for example "profile.sketch" or "body".
    pub field: String,
}

/// An asset a feature reads and the field that names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetLink {
    /// The asset file name in the document's `assets/` folder (file-format.md).
    pub name: String,
    /// Path of the params field that names it, for example "asset".
    pub field: String,
}

/// One expression a feature declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExprUse {
    /// Field path inside params, for example "extent.expr". Unique within the feature.
    pub field: String,
    /// The expression source.
    pub expr: Expr,
    /// How the value is interpreted and converted (section 3.4).
    pub kind: FieldUnit,
}

impl Dependencies {
    /// Declares a feature read at `field`; a feature already declared keeps its first field.
    pub fn add_feature(&mut self, feature: &Id, field: &str) {
        if !self.features.iter().any(|link| link.id == *feature) {
            self.features.push(link(feature, field));
        }
    }

    /// Declares a body read at `field`; a body already read or written keeps its first entry.
    pub fn add_body_read(&mut self, body: &Id, field: &str) {
        let declared = |links: &[Link]| links.iter().any(|link| link.id == *body);
        if !declared(&self.bodies_read) && !declared(&self.bodies_written) {
            self.bodies_read.push(link(body, field));
        }
    }

    /// Declares a body written at `field`. A body declared as read before moves to the written
    /// list with its first field, so no id appears in both lists.
    pub fn add_body_written(&mut self, body: &Id, field: &str) {
        if self.bodies_written.iter().any(|link| link.id == *body) {
            return;
        }
        match self.bodies_read.iter().position(|link| link.id == *body) {
            Some(position) => {
                let read = self.bodies_read.remove(position);
                self.bodies_written.push(read);
            }
            None => self.bodies_written.push(link(body, field)),
        }
    }

    /// Declares the expression at `field`, evaluated as `kind`.
    pub fn add_expression(&mut self, field: &str, expr: &Expr, kind: FieldUnit) {
        self.expressions.push(ExprUse {
            field: field.to_string(),
            expr: expr.clone(),
            kind,
        });
    }

    /// Declares what the plane at `field` reads (section 2.4): a datum plane's feature, or the
    /// body of a face.
    pub fn add_plane(&mut self, field: &str, plane: &PlaneRef) {
        match plane {
            PlaneRef::Origin { .. } => {}
            PlaneRef::Datum { feature } => self.add_feature(feature, &format!("{field}.feature")),
            PlaneRef::Face { body, .. } => self.add_body_read(body, &format!("{field}.body")),
        }
    }

    /// Declares what the axis at `field` reads (section 2.4): a sketch line's sketch, or the
    /// body of an edge or face.
    pub fn add_axis(&mut self, field: &str, axis: &AxisRef) {
        match axis {
            AxisRef::Origin { .. } => {}
            AxisRef::SketchLine { sketch, .. } => {
                self.add_feature(sketch, &format!("{field}.sketch"));
            }
            AxisRef::Edge { body, .. } | AxisRef::Face { body, .. } => {
                self.add_body_read(body, &format!("{field}.body"));
            }
        }
    }
}

/// A link to `id` named at `field`.
fn link(id: &Id, field: &str) -> Link {
    Link {
        id: id.clone(),
        field: field.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use partsmith_naming::Reference;
    use partsmith_sketch::OriginPlane;

    use super::*;
    use crate::params::OriginAxis;

    fn id(text: &str) -> Id {
        Id::new(text).expect("test ids are valid")
    }

    fn ids(links: &[Link]) -> Vec<(&str, &str)> {
        links
            .iter()
            .map(|link| (link.id.as_str(), link.field.as_str()))
            .collect()
    }

    fn face() -> Reference {
        serde_json::from_value(serde_json::json!({"q": "f-box1/top", "fp": "plane|n=0,0,1"}))
            .expect("the reference is valid")
    }

    #[test]
    fn a_feature_is_declared_once_with_its_first_field() {
        let mut dependencies = Dependencies::default();

        dependencies.add_feature(&id("f-sk1"), "profile.sketch");
        dependencies.add_feature(&id("f-sk1"), "axis.sketch");

        assert_eq!(ids(&dependencies.features), [("f-sk1", "profile.sketch")]);
    }

    #[test]
    fn a_written_body_is_not_also_read() {
        let mut dependencies = Dependencies::default();

        dependencies.add_body_written(&id("b-1"), "body");
        dependencies.add_body_read(&id("b-1"), "plane.body");

        assert_eq!(ids(&dependencies.bodies_written), [("b-1", "body")]);
        assert!(dependencies.bodies_read.is_empty());
    }

    #[test]
    fn a_read_body_that_is_then_written_moves_to_the_written_list() {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_read(&id("b-2"), "plane.body");
        dependencies.add_body_read(&id("b-1"), "tools[0]");

        dependencies.add_body_written(&id("b-2"), "target");

        assert_eq!(ids(&dependencies.bodies_written), [("b-2", "plane.body")]);
        assert_eq!(ids(&dependencies.bodies_read), [("b-1", "tools[0]")]);
    }

    #[test]
    fn expressions_keep_their_declaration_order() {
        let mut dependencies = Dependencies::default();

        dependencies.add_expression("size[1]", &Expr::new("2"), FieldUnit::Length);
        dependencies.add_expression("size[0]", &Expr::new("1"), FieldUnit::Length);

        let fields: Vec<&str> = dependencies
            .expressions
            .iter()
            .map(|expression| expression.field.as_str())
            .collect();
        assert_eq!(fields, ["size[1]", "size[0]"]);
    }

    #[test]
    fn a_datum_plane_declares_its_feature() {
        let mut dependencies = Dependencies::default();

        dependencies.add_plane(
            "plane",
            &PlaneRef::Datum {
                feature: id("f-dp1"),
            },
        );

        assert_eq!(ids(&dependencies.features), [("f-dp1", "plane.feature")]);
    }

    #[test]
    fn a_face_plane_reads_its_body() {
        let mut dependencies = Dependencies::default();

        dependencies.add_plane(
            "plane",
            &PlaneRef::Face {
                body: id("b-1"),
                face: face(),
            },
        );

        assert_eq!(ids(&dependencies.bodies_read), [("b-1", "plane.body")]);
    }

    #[test]
    fn an_origin_plane_reads_nothing() {
        let mut dependencies = Dependencies::default();

        dependencies.add_plane(
            "plane",
            &PlaneRef::Origin {
                plane: OriginPlane::Xz,
            },
        );

        assert_eq!(dependencies, Dependencies::default());
    }

    #[test]
    fn axes_declare_their_sketch_or_body() {
        let mut dependencies = Dependencies::default();

        dependencies.add_axis(
            "axis",
            &AxisRef::Origin {
                axis: OriginAxis::Z,
            },
        );
        dependencies.add_axis(
            "axis",
            &AxisRef::SketchLine {
                sketch: id("f-sk1"),
                line: id("e-9"),
            },
        );
        dependencies.add_axis(
            "second",
            &AxisRef::Face {
                body: id("b-2"),
                face: face(),
            },
        );

        assert_eq!(ids(&dependencies.features), [("f-sk1", "axis.sketch")]);
        assert_eq!(ids(&dependencies.bodies_read), [("b-2", "second.body")]);
    }
}
