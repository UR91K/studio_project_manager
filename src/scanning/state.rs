/// Represents what type of data we're currently scanning
#[derive(Debug, Clone, PartialEq)]
pub enum ScannerState {
    Root,
    
    // Sample scanning states
    InSampleRef {
        version: u32,
    },
    InSampleData {
        current_data: String,
    },
    
    // Plugin states
    InSourceContext,
    InValue,
    InBranchSourceContext,
    InPluginDesc {
        device_id: String,
    },
    InVst3PluginInfo,
    InVstPluginInfo,
    
    // Tempo states
    InTempo {
        version: u32,
    },
    InTempoManual,
    
    // Time signature state
    InTimeSignature,
} 