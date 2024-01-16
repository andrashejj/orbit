import { defineStore } from 'pinia';
import { useDisplay } from 'vuetify';
import { en as designSystemFallbackMessages } from 'vuetify/locale';
import { appInitConfig } from '~/configs';
import { Locale } from '~/configs/i18n';
import { logger } from '~/core';
import { PolicyType, SupportedTheme } from '~/types';
import { fetchDesignSystemLocale, i18n, services } from '~/ui/modules';
import { useSessionStore } from '~/ui/stores/session';
import { GlobalNotification } from '~/ui/types';

export interface AppStoreState {
  initialized: boolean;
  appName: string;
  theme: SupportedTheme;
  showSidebar: boolean;
  notification: GlobalNotification;
}

export const useAppStore = defineStore('app', {
  state: (): AppStoreState => {
    return {
      initialized: false,
      appName: appInitConfig.name,
      theme: services().theme.resolveTheme(),
      showSidebar: true,
      notification: {
        show: false,
        message: '',
        type: 'info',
      },
    };
  },
  getters: {
    isMobile(): boolean {
      const { mobile } = useDisplay();

      return mobile.value;
    },
    isDarkTheme(): boolean {
      return this.theme === SupportedTheme.Dark;
    },
    supportedLocales(): Locale[] {
      return services().locales.supportedLocales.sort();
    },
    locale(): Locale {
      return i18n.global.locale.value as Locale;
    },
    baseUrl(): string {
      return services().routes.baseUrl + this.locale;
    },
    policyTypes(): string[] {
      return Object.values(PolicyType);
    },
  },
  actions: {
    async initialize(): Promise<void> {
      if (this.initialized) {
        return;
      }

      const session = useSessionStore();
      await session.initialize();

      this.initialized = true;
    },
    async useLocale(locale: Locale, persist = false): Promise<void> {
      const isLoadedLocale = i18n.global.availableLocales.includes(locale);
      if (isLoadedLocale && i18n.global.locale.value === locale) {
        // do nothing if the locale is the same
        return;
      }

      if (!isLoadedLocale) {
        const messages = await services().locales.fetchLocaleMessages(locale);
        const designSystemMessages = await fetchDesignSystemLocale(locale).catch(e => {
          logger.error(`Failed to load design system locale for ${locale}`, e);

          return designSystemFallbackMessages;
        });
        i18n.global.setLocaleMessage(locale, {
          ...messages,
          $vuetify: designSystemMessages,
        });
      }

      i18n.global.locale.value = locale;
      services().locales.updatePageLocale(locale);
      if (persist) {
        await services().locales.saveLocale(locale);
      }
    },
    async toogleTheme(): Promise<void> {
      const theme = this.isDarkTheme ? SupportedTheme.Light : SupportedTheme.Dark;
      this.theme = theme;

      services().theme.updateUserTheme(theme);
    },
    toogleSidebar(): void {
      this.showSidebar = !this.showSidebar;
    },
    sendNotification({
      message,
      type,
    }: {
      message: GlobalNotification['message'];
      type: GlobalNotification['type'];
    }): void {
      this.notification = {
        show: true,
        message,
        type,
      };
    },
  },
});