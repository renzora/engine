module.exports = {
  plugins: {
    tailwindcss: {},
    cssnano: {
      preset: ['default', {
        discardComments: { removeAll: true },
        cssDeclarationSorter: true,
        colormin: true,
        normalizeWhitespace: true,
        minifySelectors: true,
        minifyParams: true,
        mergeLonghand: true,
        mergeRules: true,
        calc: true,
        convertValues: true,
        discardDuplicates: true,
        discardOverridden: true,
        minifyFontValues: true,
        minifyGradients: true,
        normalizePositions: true,
        normalizeRepeatStyle: true,
        normalizeString: true,
        normalizeTimingFunctions: true,
        normalizeUnicode: true,
        orderedValues: true,
        reduceInitial: true,
        reduceTransforms: true,
        svgo: true,
        uniqueSelectors: true
      }]
    }
  }
};