import { __i18n_c752d2653f2aaab9b7c61cf4bd968ceb061d3542515a7e528e6fc42c3821bf76 } from "../../.cache/translations.i18n?dev";
export const buildTranslatorByLanguage = (language)=>({
        __: (key, ...interpolations)=>__byLanguage(key, language, ...interpolations),
        __icu: (key, icuMessageData)=>__icuByLanguage(key, language, icuMessageData)
    });
const greeting = (lang)=>{
    const { __ } = buildTranslatorByLanguage(lang);
    return __(__i18n_c752d2653f2aaab9b7c61cf4bd968ceb061d3542515a7e528e6fc42c3821bf76 || "Hello [0]!", "World");
};
