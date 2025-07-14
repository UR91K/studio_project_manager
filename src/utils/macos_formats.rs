use std::path::PathBuf;
use std::collections::HashMap;
use chrono::{DateTime, Utc, TimeZone};
use uuid::Uuid;

// Mac epoch is January 1, 1904
const MAC_EPOCH: i64 = -2082844800; // Seconds from Unix epoch to Mac epoch

// Alias format constants
pub const ALIAS_KIND_FILE: u16 = 0;
pub const ALIAS_KIND_FOLDER: u16 = 1;

pub const ALIAS_HFS_VOLUME_SIGNATURE: &[u8; 2] = b"H+";

pub const ALIAS_FIXED_DISK: u16 = 0;
pub const ALIAS_NETWORK_DISK: u16 = 1;
pub const ALIAS_400KB_FLOPPY_DISK: u16 = 2;
pub const ALIAS_800KB_FLOPPY_DISK: u16 = 3;
pub const ALIAS_1_44MB_FLOPPY_DISK: u16 = 4;
pub const ALIAS_EJECTABLE_DISK: u16 = 5;

pub const ALIAS_NO_CNID: u32 = 0xFFFFFFFF;

// Bookmark format constants
pub const BMK_DATA_TYPE_MASK: u32 = 0xFFFFFF00;
pub const BMK_DATA_SUBTYPE_MASK: u32 = 0x000000FF;

pub const BMK_STRING: u32 = 0x0100;
pub const BMK_DATA: u32 = 0x0200;
pub const BMK_NUMBER: u32 = 0x0300;
pub const BMK_DATE: u32 = 0x0400;
pub const BMK_BOOLEAN: u32 = 0x0500;
pub const BMK_ARRAY: u32 = 0x0600;
pub const BMK_DICT: u32 = 0x0700;
pub const BMK_UUID: u32 = 0x0800;
pub const BMK_URL: u32 = 0x0900;
pub const BMK_NULL: u32 = 0x0A00;

// Bookmark keys
pub const K_BOOKMARK_PATH: u32 = 0x1004;
pub const K_BOOKMARK_CNID_PATH: u32 = 0x1005;
pub const K_BOOKMARK_FILE_NAME: u32 = 0x1020;
pub const K_BOOKMARK_VOLUME_PATH: u32 = 0x2002;
pub const K_BOOKMARK_VOLUME_URL: u32 = 0x2005;
pub const K_BOOKMARK_VOLUME_NAME: u32 = 0x2010;
pub const K_BOOKMARK_VOLUME_UUID: u32 = 0x2011;
pub const K_BOOKMARK_POSIX_PATH: u32 = 0x3000;

// Alias tag constants (from Python mac_alias library)
pub const TAG_CARBON_FOLDER_NAME: i16 = 0x0001;
pub const TAG_CNID_PATH: i16 = 0x0002;
pub const TAG_CARBON_PATH: i16 = 0x0003;
pub const TAG_APPLESHARE_ZONE: i16 = 0x0004;
pub const TAG_APPLESHARE_SERVER_NAME: i16 = 0x0005;
pub const TAG_APPLESHARE_USERNAME: i16 = 0x0006;
pub const TAG_DRIVER_NAME: i16 = 0x0007;
pub const TAG_NETWORK_MOUNT_INFO: i16 = 0x0008;
pub const TAG_DIALUP_INFO: i16 = 0x0009;
pub const TAG_UNICODE_FILENAME: i16 = 0x000A;
pub const TAG_UNICODE_VOLUME_NAME: i16 = 0x000B;
pub const TAG_HIGH_RES_VOLUME_CREATION_DATE: i16 = 0x000C;
pub const TAG_HIGH_RES_CREATION_DATE: i16 = 0x000D;
pub const TAG_POSIX_PATH: i16 = 0x000E;
pub const TAG_POSIX_PATH_TO_MOUNTPOINT: i16 = 0x000F;
pub const TAG_RECURSIVE_ALIAS_OF_DISK_IMAGE: i16 = 0x0010;
pub const TAG_USER_HOME_LENGTH_PREFIX: i16 = 0x0011;

#[derive(Debug, Clone, PartialEq)]
pub enum MacFormat {
    Alias(MacAlias),
    Bookmark(MacBookmark),
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppleShareInfo {
    pub zone: Option<String>,
    pub server: Option<String>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VolumeInfo {
    pub name: String,
    pub creation_date: DateTime<Utc>,
    pub fs_type: Vec<u8>,
    pub disk_type: u16,
    pub attribute_flags: u32,
    pub fs_id: Vec<u8>,
    pub appleshare_info: Option<AppleShareInfo>,
    pub driver_name: Option<Vec<u8>>,
    pub posix_path: Option<String>,
    pub disk_image_alias: Option<Box<MacAlias>>,
    pub dialup_info: Option<Vec<u8>>,
    pub network_mount_info: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetInfo {
    pub kind: u16,
    pub filename: String,
    pub folder_cnid: u32,
    pub cnid: u32,
    pub creation_date: DateTime<Utc>,
    pub creator_code: Option<Vec<u8>>,
    pub type_code: Option<Vec<u8>>,
    pub levels_from: i16,
    pub levels_to: i16,
    pub folder_name: Option<String>,
    pub cnid_path: Option<Vec<u32>>,
    pub carbon_path: Option<Vec<u8>>,
    pub posix_path: Option<String>,
    pub user_home_prefix_len: Option<i16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacAlias {
    pub appinfo: Vec<u8>,
    pub version: u16,
    pub volume: VolumeInfo,
    pub target: TargetInfo,
    pub extra: Vec<(i16, Vec<u8>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BookmarkValue {
    String(String),
    Data(Vec<u8>),
    Number(i64),
    Float(f64),
    Boolean(bool),
    Date(DateTime<Utc>),
    Uuid(Uuid),
    Url(String),
    Array(Vec<BookmarkValue>),
    Dict(HashMap<String, BookmarkValue>),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacBookmark {
    pub tocs: Vec<(u32, HashMap<u32, BookmarkValue>)>,
}

impl MacBookmark {
    pub fn get(&self, key: u32) -> Option<&BookmarkValue> {
        for (_, toc) in &self.tocs {
            if let Some(value) = toc.get(&key) {
                return Some(value);
            }
        }
        None
    }

    pub fn get_path(&self) -> Option<String> {
        if let Some(BookmarkValue::Array(path_components)) = self.get(K_BOOKMARK_PATH) {
            // Convert path components array to string path
            let mut path_parts = Vec::new();
            for component in path_components {
                if let BookmarkValue::String(s) = component {
                    path_parts.push(s.clone());
                }
            }
            if !path_parts.is_empty() {
                return Some(path_parts.join("/"));
            }
        }
        None
    }

    pub fn get_posix_path(&self) -> Option<String> {
        if let Some(BookmarkValue::String(path)) = self.get(K_BOOKMARK_POSIX_PATH) {
            return Some(path.clone());
        }
        None
    }
}

pub fn detect_mac_format(data: &[u8]) -> Result<MacFormat, String> {
    if data.len() < 16 {
        return Err("Data too short to be a valid Mac format".to_string());
    }

    // Check for bookmark format
    if data.len() >= 4 {
        let magic = &data[0..4];
        if magic == b"book" || magic == b"alis" {
            return Ok(MacFormat::Bookmark(MacBookmark { tocs: Vec::new() }));
        }
    }

            // Check for alias format
        if data.len() >= 8 {
            let appinfo = &data[0..4];
            let _recsize = u16::from_be_bytes([data[4], data[5]]);
            let version = u16::from_be_bytes([data[6], data[7]]);
            
            if version == 2 || version == 3 {
                // Try to parse the alias
                match parse_mac_alias(data) {
                    Ok(alias) => return Ok(MacFormat::Alias(alias)),
                    Err(_) => {
                        // If parsing fails, still return a basic alias structure
                        return Ok(MacFormat::Alias(MacAlias {
                            appinfo: appinfo.to_vec(),
                            version,
                            volume: VolumeInfo {
                                name: String::new(),
                                creation_date: Utc::now(),
                                fs_type: Vec::new(),
                                disk_type: 0,
                                attribute_flags: 0,
                                fs_id: Vec::new(),
                                appleshare_info: None,
                                driver_name: None,
                                posix_path: None,
                                disk_image_alias: None,
                                dialup_info: None,
                                network_mount_info: None,
                            },
                            target: TargetInfo {
                                kind: 0,
                                filename: String::new(),
                                folder_cnid: 0,
                                cnid: 0,
                                creation_date: Utc::now(),
                                creator_code: None,
                                type_code: None,
                                levels_from: -1,
                                levels_to: -1,
                                folder_name: None,
                                cnid_path: None,
                                carbon_path: None,
                                posix_path: None,
                                user_home_prefix_len: None,
                            },
                            extra: Vec::new(),
                        }));
                    }
                }
            }
        }

    Ok(MacFormat::Unknown)
}

pub fn decode_mac_path(data: &[u8]) -> Result<PathBuf, String> {
    let format = detect_mac_format(data)?;
    
    match format {
        MacFormat::Alias(alias) => {
            // Extract path from alias
            extract_alias_path(&alias)
        }
        MacFormat::Bookmark(bookmark) => {
            // Try to get POSIX path first, then fall back to path components
            if let Some(posix_path) = bookmark.get_posix_path() {
                return Ok(PathBuf::from(posix_path));
            }
            
            if let Some(path) = bookmark.get_path() {
                return Ok(PathBuf::from(path));
            }
            
            Err("No path found in bookmark".to_string())
        }
        MacFormat::Unknown => {
            Err("Unknown Mac format".to_string())
        }
    }
}

/// Parse Mac OS Alias data into a MacAlias struct
fn parse_mac_alias(data: &[u8]) -> Result<MacAlias, String> {
    if data.len() < 150 {
        return Err("Alias data too short".to_string());
    }

    let mut offset = 0;

    // Parse header (8 bytes)
    if data.len() < offset + 8 {
        return Err("Not enough data for alias header".to_string());
    }

    let appinfo = data[offset..offset + 4].to_vec();
    let recsize = u16::from_be_bytes([data[offset + 4], data[offset + 5]]);
    let version = u16::from_be_bytes([data[offset + 6], data[offset + 7]]);
    offset += 8;

    if recsize < 150 {
        return Err("Incorrect alias length".to_string());
    }

    if version != 2 && version != 3 {
        return Err(format!("Unsupported alias version {}", version));
    }

    // Parse version-specific data
    let (volume, target) = if version == 2 {
        parse_alias_v2_data(&data[offset..])?
    } else {
        parse_alias_v3_data(&data[offset..])?
    };

    let mut alias = MacAlias {
        appinfo,
        version,
        volume,
        target,
        extra: Vec::new(),
    };

    // Parse extended data tags
    parse_alias_tags(&mut alias, data)?;

    Ok(alias)
}

/// Parse version 2 alias data
#[allow(unused_assignments)]
fn parse_alias_v2_data(data: &[u8]) -> Result<(VolumeInfo, TargetInfo), String> {
    if data.len() < 142 {
        return Err("Not enough data for v2 alias".to_string());
    }

    let mut offset = 0;

    // Parse target info (142 bytes)
    let kind = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    // Volume name (28 bytes, null-terminated)
    let volname_end = data[offset..offset + 28].iter().position(|&b| b == 0).unwrap_or(28);
    let volname = String::from_utf8_lossy(&data[offset..offset + volname_end]).to_string();
    offset += 28;

    let voldate = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
    offset += 4;

    let fstype = data[offset..offset + 2].to_vec();
    offset += 2;

    let disktype = u16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let folder_cnid = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
    offset += 4;

    // Filename (64 bytes, null-terminated)
    let filename_end = data[offset..offset + 64].iter().position(|&b| b == 0).unwrap_or(64);
    let filename = String::from_utf8_lossy(&data[offset..offset + filename_end]).to_string();
    offset += 64;

    let cnid = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
    offset += 4;

    let crdate = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
    offset += 4;

    let creator_code = data[offset..offset + 4].to_vec();
    offset += 4;

    let type_code = data[offset..offset + 4].to_vec();
    offset += 4;

    let levels_from = i16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let levels_to = i16::from_be_bytes([data[offset], data[offset + 1]]);
    offset += 2;

    let volattrs = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
    offset += 4;

    let volfsid = data[offset..offset + 2].to_vec();
    offset += 2;

    // Reserved (10 bytes)
    offset += 10; //TODO: check what this is for

    let volume = VolumeInfo {
        name: volname.replace("/", ":"),
        creation_date: mac_timestamp_to_datetime(voldate),
        fs_type: fstype,
        disk_type: disktype,
        attribute_flags: volattrs,
        fs_id: volfsid,
        appleshare_info: None,
        driver_name: None,
        posix_path: None,
        disk_image_alias: None,
        dialup_info: None,
        network_mount_info: None,
    };

    let target = TargetInfo {
        kind,
        filename: filename.replace("/", ":"),
        folder_cnid,
        cnid,
        creation_date: mac_timestamp_to_datetime(crdate),
        creator_code: Some(creator_code),
        type_code: Some(type_code),
        levels_from,
        levels_to,
        folder_name: None,
        cnid_path: None,
        carbon_path: None,
        posix_path: None,
        user_home_prefix_len: None,
    };

    Ok((volume, target))
}

/// Parse version 3 alias data (simplified for now)
#[allow(unused_variables)]
fn parse_alias_v3_data(data: &[u8]) -> Result<(VolumeInfo, TargetInfo), String> {
    // TODO: Implement v3 parsing
    Err("Version 3 alias parsing not yet implemented".to_string())
}

/// Parse extended data tags
fn parse_alias_tags(alias: &mut MacAlias, data: &[u8]) -> Result<(), String> {
    // Start after the main alias structure (8 + 142 = 150 bytes for v2)
    let mut offset = 150;
    
    while offset + 2 <= data.len() {
        // Read tag
        let tag = i16::from_be_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        
        if tag == -1 {
            break; // End of tags
        }
        
        if offset + 2 > data.len() {
            break; // Not enough data for length
        }
        
        // Read length
        let length = i16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
        
        if offset + length > data.len() {
            break; // Not enough data for value
        }
        
        // Read value
        let value = data[offset..offset + length].to_vec();
        offset += length;
        
        // Handle padding (odd lengths are padded)
        if length & 1 != 0 && offset < data.len() {
            offset += 1;
        }
        
        // Process tag
        match tag {
            TAG_POSIX_PATH => {
                // POSIX path is stored as UTF-8 string
                if let Ok(path_str) = String::from_utf8(value) {
                    alias.target.posix_path = Some(path_str);
                }
            }
            0x0012 => {
                // Tag 18 appears to contain a full path - store it as an extra field
                // This might be a Carbon path or some other path format
                alias.extra.push((tag, value.clone()));
            }
            TAG_UNICODE_FILENAME => {
                // Unicode filename (skip 2-byte length prefix)
                if value.len() >= 2 {
                    let unicode_data = &value[2..];
                    // Try UTF-16BE decoding
                    let (cow, _, _) = encoding_rs::UTF_16BE.decode(unicode_data);
                    alias.target.filename = cow.to_string();
                }
            }
            TAG_UNICODE_VOLUME_NAME => {
                // Unicode volume name (skip 2-byte length prefix)
                if value.len() >= 2 {
                    let unicode_data = &value[2..];
                    // Try UTF-16BE decoding
                    let (cow, _, _) = encoding_rs::UTF_16BE.decode(unicode_data);
                    alias.volume.name = cow.to_string();
                }
            }
            TAG_CARBON_FOLDER_NAME => {
                // Carbon folder name
                if let Ok(folder_name) = String::from_utf8(value) {
                    alias.target.folder_name = Some(folder_name);
                }
            }
            TAG_CARBON_PATH => {
                // Carbon path
                alias.target.carbon_path = Some(value);
            }
            _ => {
                // Store unknown tags in extra
                alias.extra.push((tag, value));
            }
        }
    }
    
    Ok(())
}

/// Convert Mac timestamp to DateTime
fn mac_timestamp_to_datetime(timestamp: u32) -> DateTime<Utc> {
    let unix_timestamp = MAC_EPOCH + timestamp as i64;
    match Utc.timestamp_opt(unix_timestamp, 0) {
        chrono::LocalResult::Single(dt) => dt,
        _ => Utc::now(),
    }
}

/// Extract path from Mac Alias
fn extract_alias_path(alias: &MacAlias) -> Result<PathBuf, String> {
    // 1. Try tag 18 (full path)
    for (tag, value) in &alias.extra {
        if *tag == 0x0012 {
            if let Ok(path_str) = String::from_utf8(value.to_vec()) {
                return Ok(PathBuf::from(path_str));
            }
        }
    }

    // 2. Try POSIX path
    if let Some(posix_path) = &alias.target.posix_path {
        let bytes = posix_path.as_bytes();
        match crate::utils::samples::decode_posix_path_bytes(bytes) {
            Ok(decoded_path) => return Ok(PathBuf::from(decoded_path)),
            Err(_) => return Ok(PathBuf::from(posix_path)),
        }
    }

    // 3. Try Carbon path (not implemented)
    if let Some(_carbon_path) = &alias.target.carbon_path {
        return Err("Carbon path decoding not yet implemented".to_string());
    }

    // 4. Fallback to volume + filename
    if !alias.volume.name.is_empty() && !alias.target.filename.is_empty() {
        let path = format!("/Volumes/{}/{}", alias.volume.name, alias.target.filename);
        return Ok(PathBuf::from(path));
    }

    Err("No path information found in alias".to_string())
} 