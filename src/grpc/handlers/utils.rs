use crate::database::LiveSetDatabase;
use crate::error::DatabaseError;
use crate::grpc::proto::{AbletonVersion, KeySignature, Plugin, Project, Sample, TimeSignature};
use crate::live_set::LiveSet;

pub fn convert_live_set_to_proto(
    live_set: LiveSet,
    db: &mut LiveSetDatabase,
) -> Result<Project, DatabaseError> {
    let project_id = live_set.id.to_string();

    // Load notes from database
    let notes = db.get_project_notes(&project_id)?.unwrap_or_default();

    // Load audio file ID from database
    let audio_file_id = db
        .get_project_audio_file(&project_id)?
        .map(|media_file| media_file.id);

    // Load collection associations from database
    let collection_ids = db.get_collections_for_project(&project_id)?;

    // Load tag data from database
    let tag_data = db.get_project_tag_data(&project_id)?;

    // Load tasks from database
    let tasks = db
        .get_project_tasks(&project_id)?
        .into_iter()
        .map(
            |(task_id, description, completed, created_at)| crate::grpc::proto::Task {
                id: task_id,
                description,
                completed,
                project_id: project_id.clone(), // Add project_id to Task
                created_at,
            },
        )
        .collect();

    // Convert tags with proper IDs and creation timestamps
    let tags = tag_data
        .into_iter()
        .map(|(tag_id, tag_name, created_at)| crate::grpc::proto::Tag {
            id: tag_id,
            name: tag_name,
            created_at,
        })
        .collect();

    Ok(Project {
        id: project_id,
        name: live_set.name,
        path: live_set.file_path.to_string_lossy().to_string(),
        hash: live_set.file_hash,
        notes,
        created_at: live_set.created_time.timestamp(),
        modified_at: live_set.modified_time.timestamp(),
        last_parsed_at: live_set.last_parsed_timestamp.timestamp(),

        tempo: live_set.tempo,
        time_signature: Some(TimeSignature {
            numerator: live_set.time_signature.numerator as i32,
            denominator: live_set.time_signature.denominator as i32,
        }),
        key_signature: live_set.key_signature.map(|ks| KeySignature {
            tonic: ks.tonic.to_string(),
            scale: ks.scale.to_string(),
        }),
        duration_seconds: live_set.estimated_duration.map(|d| d.num_seconds() as f64),
        furthest_bar: live_set.furthest_bar,

        ableton_version: Some(AbletonVersion {
            major: live_set.ableton_version.major,
            minor: live_set.ableton_version.minor,
            patch: live_set.ableton_version.patch,
            beta: live_set.ableton_version.beta,
        }),

        plugins: live_set
            .plugins
            .into_iter()
            .map(|p| Plugin {
                id: p.id.to_string(),
                ableton_plugin_id: p.plugin_id,
                ableton_module_id: p.module_id,
                dev_identifier: p.dev_identifier,
                name: p.name,
                format: p.plugin_format.to_string(),
                installed: p.installed,
                vendor: Some(p.vendor.unwrap_or_default()),
                version: Some(p.version.unwrap_or_default()),
                sdk_version: Some(p.sdk_version.unwrap_or_default()),
                flags: p.flags,
                scanstate: p.scanstate,
                enabled: p.enabled,
            })
            .collect(),

        samples: live_set
            .samples
            .into_iter()
            .map(|s| Sample {
                id: s.id.to_string(),
                name: s.name,
                path: s.path.to_string_lossy().to_string(),
                is_present: s.is_present,
            })
            .collect(),

        tags,
        tasks,
        collection_ids,
        audio_file_id,
    })
}
