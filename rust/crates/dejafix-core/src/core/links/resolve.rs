use super::types::{Direction, Link, LinkResolution, Registered};

pub const MAX_LINKS: usize = 10;

fn names(value: &str, project: &Registered) -> bool {
    value.eq_ignore_ascii_case(&project.identity)
        || value.eq_ignore_ascii_case(&project.folder)
        || project
            .alias
            .as_deref()
            .is_some_and(|alias| value.eq_ignore_ascii_case(alias))
}

fn canonical<'p>(others: &'p [Registered], own: &Registered) -> (Vec<&'p Registered>, Vec<String>) {
    let mut sorted: Vec<&Registered> = others
        .iter()
        .filter(|project| {
            project.path != own.path && !project.identity.eq_ignore_ascii_case(&own.identity)
        })
        .collect();
    sorted.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    let mut kept: Vec<&Registered> = Vec::new();
    let mut duplicates: Vec<String> = Vec::new();
    for project in sorted {
        let Some(known) = kept
            .iter_mut()
            .find(|known| known.identity.eq_ignore_ascii_case(&project.identity))
        else {
            kept.push(project);
            continue;
        };
        let dropped = if known.worktree && !project.worktree {
            std::mem::replace(known, project)
        } else {
            project
        };
        if !dropped.worktree && !duplicates.contains(&dropped.identity) {
            duplicates.push(dropped.identity.clone());
        }
    }
    (kept, duplicates)
}

fn add(links: &mut Vec<Link>, project: &Registered, direction: Direction, relation: &str) {
    if let Some(link) = links.iter_mut().find(|link| link.path == project.path) {
        if link.direction != direction {
            link.direction = Direction::Both;
        }
        if link.relation.is_empty() {
            relation.clone_into(&mut link.relation);
        }
        return;
    }
    links.push(Link {
        id: project.id.clone(),
        identity: project.identity.clone(),
        path: project.path.clone(),
        direction,
        relation: relation.to_owned(),
        boundary: project.config.boundary.clone(),
    });
}

#[must_use]
pub fn resolve_links(own: &Registered, others: &[Registered]) -> LinkResolution {
    let (projects, duplicates) = canonical(others, own);
    let mut resolution = LinkResolution {
        duplicates,
        ..LinkResolution::default()
    };
    for spec in &own.config.links {
        if names(&spec.project, own) {
            continue;
        }
        match projects
            .iter()
            .find(|project| names(&spec.project, project))
        {
            Some(project) => add(
                &mut resolution.links,
                project,
                Direction::Declared,
                &spec.relation,
            ),
            None => {
                if !resolution.unresolved.contains(&spec.project) {
                    resolution.unresolved.push(spec.project.clone());
                }
            }
        }
    }
    for project in &projects {
        for spec in project
            .config
            .links
            .iter()
            .filter(|spec| names(&spec.project, own))
        {
            add(
                &mut resolution.links,
                project,
                Direction::Reverse,
                &spec.relation,
            );
        }
    }
    resolution.links.truncate(MAX_LINKS);
    let linked = &resolution.links;
    resolution.duplicates.retain(|identity| {
        linked
            .iter()
            .any(|link| link.identity.eq_ignore_ascii_case(identity))
    });
    resolution
}
