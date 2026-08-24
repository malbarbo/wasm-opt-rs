//! A builder API for `OptimizationOptions` and `CtorEvalOptions`.

use crate::api::*;
use crate::ctor_eval::CtorEvalOptions;

/// Builder methods.
impl OptimizationOptions {
    /// Sets [`ReaderOptions::file_type`].
    pub fn reader_file_type(&mut self, value: FileType) -> &mut Self {
        self.reader.file_type = value;
        self
    }

    /// Sets [`ReaderOptions::preserve_type_order`].
    pub fn preserve_type_order(&mut self, value: bool) -> &mut Self {
        self.reader.preserve_type_order = value;
        self
    }

    /// Sets [`WriterOptions::file_type`].
    pub fn writer_file_type(&mut self, value: FileType) -> &mut Self {
        self.writer.file_type = value;
        self
    }

    /// Sets [`OptimizationOptions::converge`].
    pub fn set_converge(&mut self) -> &mut Self {
        self.converge = true;
        self
    }

    /// Sets [`InliningOptions::always_inline_max_size`].
    pub fn always_inline_max_size(&mut self, value: u32) -> &mut Self {
        self.inlining.always_inline_max_size = value;
        self
    }

    /// Sets [`InliningOptions::one_caller_inline_max_size`].
    pub fn one_caller_inline_max_size(&mut self, value: u32) -> &mut Self {
        self.inlining.one_caller_inline_max_size = value;
        self
    }

    /// Sets [`InliningOptions::flexible_inline_max_size`].
    pub fn flexible_inline_max_size(&mut self, value: u32) -> &mut Self {
        self.inlining.flexible_inline_max_size = value;
        self
    }

    /// Sets [`InliningOptions::max_combined_binary_size`].
    pub fn max_combined_binary_size(&mut self, value: u32) -> &mut Self {
        self.inlining.max_combined_binary_size = value;
        self
    }

    /// Sets [`InliningOptions::allow_functions_with_loops`].
    pub fn allow_functions_with_loops(&mut self, value: bool) -> &mut Self {
        self.inlining.allow_functions_with_loops = value;
        self
    }

    /// Sets [`InliningOptions::partial_inlining_ifs`].
    pub fn partial_inlining_ifs(&mut self, value: u32) -> &mut Self {
        self.inlining.partial_inlining_ifs = value;
        self
    }

    /// Sets [`PassOptions::validate`].
    pub fn validate(&mut self, value: bool) -> &mut Self {
        self.passopts.validate = value;
        self
    }

    /// Sets [`PassOptions::validate_globally`].
    pub fn validate_globally(&mut self, value: bool) -> &mut Self {
        self.passopts.validate_globally = value;
        self
    }

    /// Sets [`PassOptions::optimize_level`].
    pub fn optimize_level(&mut self, value: OptimizeLevel) -> &mut Self {
        self.passopts.optimize_level = value;
        self
    }

    /// Sets [`PassOptions::shrink_level`].
    pub fn shrink_level(&mut self, value: ShrinkLevel) -> &mut Self {
        self.passopts.shrink_level = value;
        self
    }

    /// Sets [`PassOptions::traps_never_happen`].
    pub fn traps_never_happen(&mut self, value: bool) -> &mut Self {
        self.passopts.traps_never_happen = value;
        self
    }

    /// Sets [`PassOptions::low_memory_unused`].
    pub fn low_memory_unused(&mut self, value: bool) -> &mut Self {
        self.passopts.low_memory_unused = value;
        self
    }

    /// Sets [`PassOptions::fast_math`].
    pub fn fast_math(&mut self, value: bool) -> &mut Self {
        self.passopts.fast_math = value;
        self
    }

    /// Sets [`PassOptions::zero_filled_memory`].
    pub fn zero_filled_memory(&mut self, value: bool) -> &mut Self {
        self.passopts.zero_filled_memory = value;
        self
    }

    /// Sets [`PassOptions::debug_info`].
    pub fn debug_info(&mut self, value: bool) -> &mut Self {
        self.passopts.debug_info = value;
        self
    }

    /// Sets [`PassOptions::allow_stack_ir`].
    pub fn allow_stack_ir(&mut self, value: bool) -> &mut Self {
        self.passopts.allow_stack_ir = value;
        self
    }

    /// Adds a pass argument to [`PassOptions::arguments`].
    pub fn set_pass_arg(&mut self, key: &str, value: &str) -> &mut Self {
        self.passopts
            .arguments
            .insert(key.to_string(), value.to_string());
        self
    }

    /// Sets [`Passes::add_default_passes`].
    pub fn add_default_passes(&mut self, value: bool) -> &mut Self {
        self.passes.add_default_passes = value;
        self
    }

    /// Adds a pass to [`Passes::more_passes`].
    pub fn add_pass(&mut self, value: Pass) -> &mut Self {
        self.passes.more_passes.push(value);
        self
    }

    /// Sets the baseline feature set to [`FeatureBaseline::MvpOnly`].
    pub fn mvp_features_only(&mut self) -> &mut Self {
        self.features.baseline = FeatureBaseline::MvpOnly;
        self
    }

    /// Sets the baseline feature set to [`FeatureBaseline::All`].
    pub fn all_features(&mut self) -> &mut Self {
        self.features.baseline = FeatureBaseline::All;
        self
    }

    /// Enables a feature.
    ///
    /// This adds the feature to [`Features::enabled`], and is equivalent to the
    /// `--enable-{feature}` command line arguments.
    pub fn enable_feature(&mut self, feature: Feature) -> &mut Self {
        self.features.enabled.insert(feature);
        self
    }

    /// Disables a feature.
    ///
    /// This adds the feature to [`Features::disabled`], and is equivalent to
    /// the `--disable-{feature}` command line arguments.
    pub fn disable_feature(&mut self, feature: Feature) -> &mut Self {
        self.features.disabled.insert(feature);
        self
    }
}

/// Builder methods.
impl CtorEvalOptions {
    /// Sets [`ReaderOptions::file_type`].
    pub fn reader_file_type(&mut self, value: FileType) -> &mut Self {
        self.reader.file_type = value;
        self
    }

    /// Sets [`ReaderOptions::preserve_type_order`].
    pub fn preserve_type_order(&mut self, value: bool) -> &mut Self {
        self.reader.preserve_type_order = value;
        self
    }

    /// Sets [`WriterOptions::file_type`].
    pub fn writer_file_type(&mut self, value: FileType) -> &mut Self {
        self.writer.file_type = value;
        self
    }

    /// Adds an exported function to [`CtorEvalOptions::ctors`].
    pub fn add_ctor(&mut self, value: impl Into<String>) -> &mut Self {
        self.ctors.push(value.into());
        self
    }

    /// Adds an exported function to [`CtorEvalOptions::kept_exports`].
    ///
    /// It should also be added with [`CtorEvalOptions::add_ctor`];
    /// keeping the export of a ctor that is not evaluated does nothing.
    pub fn add_kept_export(&mut self, value: impl Into<String>) -> &mut Self {
        self.kept_exports.push(value.into());
        self
    }

    /// Sets [`CtorEvalOptions::ignore_external_input`].
    pub fn ignore_external_input(&mut self, value: bool) -> &mut Self {
        self.ignore_external_input = value;
        self
    }

    /// Sets [`CtorEvalOptions::debug_info`].
    pub fn debug_info(&mut self, value: bool) -> &mut Self {
        self.debug_info = value;
        self
    }

    /// Sets [`CtorEvalOptions::quiet`].
    pub fn quiet(&mut self, value: bool) -> &mut Self {
        self.quiet = value;
        self
    }

    /// Sets the baseline feature set to [`FeatureBaseline::MvpOnly`].
    pub fn mvp_features_only(&mut self) -> &mut Self {
        self.features.baseline = FeatureBaseline::MvpOnly;
        self
    }

    /// Sets the baseline feature set to [`FeatureBaseline::All`].
    pub fn all_features(&mut self) -> &mut Self {
        self.features.baseline = FeatureBaseline::All;
        self
    }

    /// Enables a feature.
    ///
    /// This adds the feature to [`Features::enabled`], and is equivalent to the
    /// `--enable-{feature}` command line arguments.
    pub fn enable_feature(&mut self, feature: Feature) -> &mut Self {
        self.features.enabled.insert(feature);
        self
    }

    /// Disables a feature.
    ///
    /// This adds the feature to [`Features::disabled`], and is equivalent to
    /// the `--disable-{feature}` command line arguments.
    pub fn disable_feature(&mut self, feature: Feature) -> &mut Self {
        self.features.disabled.insert(feature);
        self
    }
}
