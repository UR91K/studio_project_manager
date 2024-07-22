// use quick_xml::events::Event;
// use quick_xml::Reader;
// use crate::error::LiveSetError;
// 
// use std::default::Default;
// use crate::utils::StringResultExt;
// 
// pub struct ScanOptions {
//     pub scan_plugins: bool,
//     pub scan_samples: bool,
//     pub scan_tempo: bool,
//     pub scan_time_signature: bool,
//     pub scan_midi: bool,
//     pub scan_audio: bool,
//     pub scan_automation: bool,
//     pub scan_return_tracks: bool,
//     pub scan_master_track: bool,
//     pub estimate_duration: bool,
//     pub calculate_furthest_bar: bool,
// }
// 
// impl Default for ScanOptions {
//     fn default() -> Self {
//         ScanOptions {
//             scan_plugins: true,
//             scan_samples: true,
//             scan_tempo: true,
//             scan_time_signature: true,
//             scan_midi: true,
//             scan_audio: true,
//             scan_automation: true,
//             scan_return_tracks: true,
//             scan_master_track: true,
//             estimate_duration: true,
//             calculate_furthest_bar: true,
//         }
//     }
// }
// 
// impl ScanOptions {
//     pub fn new() -> Self {
//         Default::default()
//     }
// 
//     pub fn plugins_only() -> Self {
//         ScanOptions {
//             scan_plugins: true,
//             ..Default::default()
//         }
//     }
// 
//     pub fn samples_only() -> Self {
//         ScanOptions {
//             scan_samples: true,
//             ..Default::default()
//         }
//     }
// 
//     
// }
// struct LiveSet {
//     // fields
// }
// impl LiveSet {
//     pub fn scan_live_set(&mut self, scan_options: ScanOptions) -> Result<(), LiveSetError> {
//         //
//         let xml_data: Vec<u8> = Vec::new();
//         //
//         let mut xml_reader = Reader::from_reader(&xml_data[..]);
//         let mut buf = Vec::new();
// 
//         loop {
//             match xml_reader.read_event(&mut buf) {
//                 Ok(Event::Start(ref event)) => {
//                     match event.name().to_str_result().unwrap_or("invalid event name") {
//                         "PluginDesc" if scan_options.scan_plugins => {
//                             self.handle_plugin_event(&mut xml_reader)?;
//                         },
//                         "SampleRef" if scan_options.scan_samples => {
//                             self.handle_sample_event(&mut xml_reader)?;
//                         },
//                         "Tempo" if scan_options.scan_tempo => {
//                             self.handle_tempo_event(&mut xml_reader)?;
//                         },
// 
//                         //etc
//                         _ => {},
//                     }
//                 },
//                 Ok(Event::Eof) => break,
//                 Err(e) => return Err(LiveSetError::XmlError(e)),
//                 _ => {},
//             }
//             buf.clear();
//         }
// 
//         Ok(())
//     }
// }