// At the top of the file, add the default theme XML as a constant with triple # delimiter
pub(crate) const DEFAULT_THEME_XML: &str = r###"
<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12049" SchemaChangeCount="3" Creator="Ableton Live 12.0d1" Revision="">
	<Theme>
		<ControlForeground Value="#b5b5b5" />
		<TextDisabled Value="#757575" />
		<ControlDisabled Value="#757575" />
		<MeterBackground Value="#181818" />
		<SurfaceHighlight Value="#464646" />
		<SurfaceArea Value="#242424" />
		<Desktop Value="#2a2a2a" />
		<ViewCheckControlEnabledOn Value="#ffad56" />
		<ScrollbarInnerHandle Value="#696969" />
		<ScrollbarInnerTrack Value="#00000000" />
		<ScrollbarLCDHandle Value="#696969" />
		<ScrollbarLCDTrack Value="#363636" />
		<ScrollbarMixerShowOnScrollHandle Value="#00000066" />
		<DetailViewBackground Value="#3e3e3e" />
		<PreferencesTab Value="#363636" />
		<SelectionFrame Value="#757575" />
		<ControlBackground Value="#1e1e1e" />
		<ControlFillHandle Value="#5d5d5d" />
		<ChosenDefault Value="#ffad56" />
		<ChosenRecord Value="#ff5559" />
		<ChosenPreListen Value="#3c6ab6" />
		<ImplicitArm Value="#ff000064" />
		<RangeDefault Value="#03c3d5" />
		<RangeDisabled Value="#757575" />
		<RangeDisabledOff Value="#636363" />
		<LearnMidi Value="#4034ef" />
		<LearnKey Value="#ff6400" />
		<LearnMacro Value="#00da48" />
		<RangeEditField Value="#007383" />
		<RangeEditField2 Value="#a03c4c" />
		<BipolReset Value="#03c3d5" />
		<ChosenAlternative Value="#03c3d5" />
		<ChosenAlert Value="#e76942" />
		<ChosenPlay Value="#00d38d" />
		<Clip1 Value="#8b7936" />
		<Clip2 Value="#999565" />
		<Clip3 Value="#b8ce93" />
		<Clip4 Value="#afb95b" />
		<Clip5 Value="#52ba46" />
		<Clip6 Value="#81d24c" />
		<Clip7 Value="#6baace" />
		<Clip8 Value="#4881aa" />
		<Clip9 Value="#954eb2" />
		<Clip10 Value="#ff5f80" />
		<Clip11 Value="#dc4848" />
		<Clip12 Value="#d66b18" />
		<Clip13 Value="#e0aa2a" />
		<Clip14 Value="#ffec75" />
		<Clip15 Value="#e7e6e6" />
		<Clip16 Value="#a0a0a0" />
		<ClipText Value="#000000" />
		<ClipBorder Value="#2a2a2a" />
		<SceneContrast Value="#2a2a2a" />
		<SelectionBackground Value="#b0ddeb" />
		<StandbySelectionBackground Value="#637e86" />
		<SelectionForeground Value="#070707" />
		<StandbySelectionForeground Value="#070707" />
		<SelectionBackgroundContrast Value="#7a959e" />
		<SurfaceBackground Value="#363636" />
		<TakeLaneTrackHighlighted Value="#3e3e3e" />
		<TakeLaneTrackNotHighlighted Value="#303030" />
		<AutomationColor Value="#ff4d47" />
		<AutomationGrid Value="#0b0b0b" />
		<LoopColor Value="#919191" />
		<OffGridLoopColor Value="#8888884f" />
		<ArrangementRulerMarkings Value="#919191" />
		<DetailViewRulerMarkings Value="#919191" />
		<ShadowDark Value="#24242454" />
		<ShadowLight Value="#2b2b2bcc" />
		<DisplayBackground Value="#181818" />
		<AbletonColor Value="#00ff00" />
		<WaveformColor Value="#171717ef" />
		<DimmedWaveformColor Value="#696969df" />
		<VelocityColor Value="#e95449" />
		<VelocitySelectedOrHovered Value="#5b8cff" />
		<NoteProbability Value="#636363" />
		<Alert Value="#e76942" />
		<ControlOnForeground Value="#070707" />
		<ControlOffForeground Value="#b5b5b5" />
		<ControlOnDisabledForeground Value="#242424" />
		<ControlOffDisabledForeground Value="#696969" />
		<ControlOnAlternativeForeground Value="#070707" />
		<ControlTextBack Value="#242424" />
		<ControlContrastFrame Value="#111111" />
		<ControlSelectionFrame Value="#919191" />
		<ControlContrastTransport Value="#363636" />
		<ViewControlOn Value="#ffad56" />
		<ViewControlOff Value="#464646" />
		<Progress Value="#ffad56" />
		<ProgressText Value="#636363" />
		<TransportProgress Value="#ffad56" />
		<ClipSlotButton Value="#1e1e1e" />
		<BrowserBar Value="#363636" />
		<BrowserBarOverlayHintTextColor Value="#5d5d5d" />
		<BrowserDisabledItem Value="#757575" />
		<BrowserSampleWaveform Value="#868686" />
		<AutomationDisabled Value="#646464" />
		<AutomationMouseOver Value="#ff9085" />
		<MidiNoteMaxVelocity Value="#e95449" />
		<RetroDisplayBackground Value="#181818" />
		<RetroDisplayBackgroundLine Value="#3e3e3e" />
		<RetroDisplayForeground Value="#ffad56" />
		<RetroDisplayForeground2 Value="#03c3d5" />
		<RetroDisplayForegroundDisabled Value="#868686" />
		<RetroDisplayGreen Value="#ffad56" />
		<RetroDisplayRed Value="#03c3d5" />
		<RetroDisplayHandle1 Value="#ffad56" />
		<RetroDisplayHandle2 Value="#ff697f" />
		<RetroDisplayScaleText Value="#868686" />
		<RetroDisplayTitle Value="#b5b5b5" />
		<ThresholdLineColor Value="#03c3d5" />
		<GainReductionLineColor Value="#ffad56" />
		<InputCurveColor Value="#535353" />
		<InputCurveOutlineColor Value="#00000000" />
		<OutputCurveColor Value="#8686864c" />
		<OutputCurveOutlineColor Value="#bfbfbf" />
		<SpectrumDefaultColor Value="#535353" />
		<SpectrumAlternativeColor Value="#3c6ab6" />
		<SpectrumGridLines Value="#b6b6b63f" />
		<Operator1 Value="#e0d825" />
		<Operator2 Value="#29d6cd" />
		<Operator3 Value="#6571f6" />
		<Operator4 Value="#f3751b" />
		<DrumRackScroller1 Value="#464646" />
		<DrumRackScroller2 Value="#696969" />
		<FilledDrumRackPad Value="#b5b5b5" />
		<SurfaceAreaFocus Value="#242424" />
		<FreezeColor Value="#4391e6" />
		<GridLabel Value="#b5b5b57f" />
		<GridLineBase Value="#06060654" />
		<ArrangerGridTiles Value="#0a0a0a19" />
		<DetailGridTiles Value="#0a0a0a19" />
		<GridGuideline Value="#b5b5b5" />
		<OffGridGuideline Value="#b5b5b54f" />
		<TreeColumnHeadBackground Value="#464646" />
		<TreeColumnHeadForeground Value="#b5b5b5" />
		<TreeColumnHeadSelected Value="#464646" />
		<TreeColumnHeadFocus Value="#464646" />
		<TreeColumnHeadControl Value="#757575" />
		<TreeRowCategoryForeground Value="#757575" />
		<TreeRowCategoryBackground Value="#00000000" />
		<BrowserTagBackground Value="#242424" />
		<SearchIndication Value="#ecca6d" />
		<KeyZoneBackground Value="#acf6b4" />
		<KeyZoneCrossfadeRamp Value="#28bd56" />
		<VelocityZoneBackground Value="#f5a7a3" />
		<VelocityZoneCrossfadeRamp Value="#e95449" />
		<SelectorZoneBackground Value="#bed6f4" />
		<SelectorZoneCrossfadeRamp Value="#2d66d2" />
		<ViewCheckControlEnabledOff Value="#757575" />
		<ViewCheckControlDisabledOn Value="#868686" />
		<ViewCheckControlDisabledOff Value="#5d5d5d" />
		<DefaultBlendFactor Value="0.7570000291" />
		<IconBlendFactor Value="0.7300000191" />
		<ClipBlendFactor Value="0.7580000162" />
		<NoteBorderStandbyBlendFactor Value="0.5360000134" />
		<RetroDisplayBlendFactor Value="1" />
		<CheckControlNotCheckedBlendFactor Value="0.5" />
		<MixSurfaceAreaBlendFactor Value="0.375" />
		<TextFrameSegmentBlendFactor Value="0.3950000107" />
		<NoteDisabledSelectedBlendFactor Value="0.5" />
		<BackgroundClip Value="#5d5d5d" />
		<BackgroundClipFrame Value="#363636" />
		<WarperTimeBarRulerBackground Value="#363636" />
		<WarperTimeBarMarkerBackground Value="#303030" />
		<MinVelocityNoteBlendFactor Value="0.2639999986" />
		<StripedBackgroundShadeFactor Value="0.8999999762" />
		<NonEditableAutomationAlpha Value="127" />
		<DisabledContextMenuIconAlpha Value="85" />
		<ClipBorderAlpha Value="120" />
		<ScrollBarAlpha Value="255" />
		<ScrollBarOnHoverAlpha Value="255" />
		<ScrollBarBackgroundAlpha Value="255" />
		<InaudibleTakeLightness Value="0.322000000000000008" />
		<InaudibleTakeSaturation Value="0.775000000000000022" />
		<InaudibleTakeNameLightness Value="0.800000000000000044" />
		<InaudibleTakeNameSaturation Value="0.853999999999999981" />
		<AutomationLaneClipBodyLightness Value="0.260000000000000009" />
		<AutomationLaneClipBodySaturation Value="0.80600000000000005" />
		<AutomationLaneHeaderLightness Value="0.251000000000000001" />
		<AutomationLaneHeaderSaturation Value="0.53400000000000003" />
		<TakeLaneHeaderLightness Value="0.479999999999999982" />
		<TakeLaneHeaderSaturation Value="0.900000000000000022" />
		<TakeLaneHeaderNameLightness Value="0.849999999999999978" />
		<TakeLaneHeaderNameSaturation Value="0.900000000000000022" />
		<AutomationLaneHeaderNameLightness Value="0.708999999999999964" />
		<AutomationLaneHeaderNameSaturation Value="0.900000000000000022" />
		<ClipContrastColorAdjustment Value="24" />
		<SessionSlotOklabLCompensationFactor Value="20" />
		<BipolarPotiTriangle Value="#111111" />
		<Poti Value="#111111" />
		<DeactivatedPoti Value="#2a2a2a" />
		<PotiNeedle Value="#181818" />
		<DeactivatedPotiNeedle Value="#2a2a2a" />
		<PianoBlackKey Value="#464646" />
		<PianoWhiteKey Value="#868686" />
		<PianoKeySeparator Value="#464646" />
		<TransportOffBackground Value="#363636" />
		<TransportOffDisabledForeground Value="#696969" />
		<TransportOffForeground Value="#b5b5b5" />
		<TransportSelectionBackground Value="#5d5d5d" />
		<Modulation Value="#009aac" />
		<ModulationDisabled Value="#79bdc7a9" />
		<ModulationMouseOver Value="#8cffff" />
		<AutomationTransformToolFrame Value="#292929bf" />
		<AutomationTransformToolFrameActive Value="#0b0b0b" />
		<AutomationTransformToolHandle Value="#292929bf" />
		<AutomationTransformToolHandleActive Value="#0b0b0b" />
		<MutedAuditionClip Value="#636363" />
		<LinkedTrackHover Value="#b3d4e5" />
		<ExpressionLaneHeaderHighlight Value="#3e3e3e" />
		<DeactivatedClipHeader Value="#696969df" />
		<DeactivatedClipHeaderForeground Value="#2a2a2a" />
		<ScaleAwareness Value="#b595fc" />
		<StandardVuMeter>
			<OnlyMinimumToMaximum Value="false" />
			<Maximum Value="#ff0a0a" />
			<AboveZeroDecibel Value="#ff0a0a" />
			<ZeroDecibel Value="#ff0a0a" />
			<BelowZeroDecibel1 Value="#ffd100" />
			<BelowZeroDecibel2 Value="#00f758" />
			<Minimum Value="#00f758" />
		</StandardVuMeter>
		<OverloadVuMeter>
			<OnlyMinimumToMaximum Value="true" />
			<Maximum Value="#ff0a0a" />
			<AboveZeroDecibel Value="#ffffff" />
			<ZeroDecibel Value="#ffffff" />
			<BelowZeroDecibel1 Value="#ffffff" />
			<BelowZeroDecibel2 Value="#ffffff" />
			<Minimum Value="#af0a0a" />
		</OverloadVuMeter>
		<DisabledVuMeter>
			<OnlyMinimumToMaximum Value="false" />
			<Maximum Value="#ff0a0a" />
			<AboveZeroDecibel Value="#ffd00a" />
			<ZeroDecibel Value="#828282" />
			<BelowZeroDecibel1 Value="#7e7e7e" />
			<BelowZeroDecibel2 Value="#7b7b7b" />
			<Minimum Value="#6e6e6e" />
		</DisabledVuMeter>
		<HeadphonesVuMeter>
			<OnlyMinimumToMaximum Value="false" />
			<Maximum Value="#a5a5f1" />
			<AboveZeroDecibel Value="#90aaec" />
			<ZeroDecibel Value="#90aaec" />
			<BelowZeroDecibel1 Value="#85b8f1" />
			<BelowZeroDecibel2 Value="#7cc6f5" />
			<Minimum Value="#0affff" />
		</HeadphonesVuMeter>
		<SendsOnlyVuMeter>
			<OnlyMinimumToMaximum Value="false" />
			<Maximum Value="#c8c800" />
			<AboveZeroDecibel Value="#c8c800" />
			<ZeroDecibel Value="#6464ff" />
			<BelowZeroDecibel1 Value="#6464ff" />
			<BelowZeroDecibel2 Value="#6464ff" />
			<Minimum Value="#6464ff" />
		</SendsOnlyVuMeter>
		<BipolarGainReductionVuMeter>
			<OnlyMinimumToMaximum Value="false" />
			<Maximum Value="#5577c6" />
			<AboveZeroDecibel Value="#5577c6" />
			<ZeroDecibel Value="#ffa519" />
			<BelowZeroDecibel1 Value="#ffa519" />
			<BelowZeroDecibel2 Value="#ffa519" />
			<Minimum Value="#ffa519" />
		</BipolarGainReductionVuMeter>
		<OrangeVuMeter>
			<OnlyMinimumToMaximum Value="true" />
			<Maximum Value="#ffa519" />
			<AboveZeroDecibel Value="#ffa519" />
			<ZeroDecibel Value="#ffa519" />
			<BelowZeroDecibel1 Value="#ffa519" />
			<BelowZeroDecibel2 Value="#ffa519" />
			<Minimum Value="#ffa519" />
		</OrangeVuMeter>
		<MainViewFocusIndicator Value="#757575" />
		<MidiEditorBlackKeyBackground Value="#07070726" />
		<MidiEditorBackgroundWhiteKeySeparator Value="#0a0a0a19" />
		<RangeEditField3 Value="#7b5732" />
		<ScrollbarInnerHandleHover Value="#757575" />
		<ScrollbarInnerTrackHover Value="#242424" />
		<ScrollbarLCDHandleHover Value="#757575" />
		<ScrollbarLCDTrackHover Value="#464646" />
		<ScrollbarMixerShowOnScrollHandleHover Value="#0000007f" />
	</Theme>
</Ableton>
"###;