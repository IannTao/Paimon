# Triage labels

| Role | GitHub label / 归档 Status 值 | Meaning |
| --- | --- | --- |
| needs-triage | needs-triage | Maintainer needs to evaluate |
| needs-info | needs-info | Waiting for reporter information |
| ready-for-agent | ready-for-agent | Fully specified for agent implementation |
| ready-for-human | ready-for-human | Requires human implementation |
| wontfix | wontfix | Will not be actioned |

When a skill mentions a role or label, apply the matching GitHub label in `IannTao/Paimon`. Preserve unrelated labels. Triage labels describe readiness, not execution progress: use GitHub open/closed state, closure reason, and the issue's `Lifecycle` field for progress. Local Markdown `Status` fields are migration snapshots and no longer determine live state.
