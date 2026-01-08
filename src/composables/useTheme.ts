import { ref, watch, onMounted } from 'vue';

export type ThemeMode = 'light' | 'dark' | 'system';

const STORAGE_KEY = 'indibudget-theme';

// Global state (shared across components)
const currentTheme = ref<ThemeMode>('system');
const isDark = ref(false);

function getSystemTheme(): boolean {
  if (typeof window === 'undefined') return false;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

function applyTheme(dark: boolean) {
  isDark.value = dark;
  if (dark) {
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.classList.remove('dark');
  }
}

function updateTheme() {
  if (currentTheme.value === 'system') {
    applyTheme(getSystemTheme());
  } else {
    applyTheme(currentTheme.value === 'dark');
  }
}

export function useTheme() {
  const setTheme = (theme: ThemeMode) => {
    currentTheme.value = theme;
    localStorage.setItem(STORAGE_KEY, theme);
    updateTheme();
  };

  const initTheme = () => {
    // Load saved preference
    const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
    if (saved && ['light', 'dark', 'system'].includes(saved)) {
      currentTheme.value = saved;
    }

    // Apply theme
    updateTheme();

    // Listen for system theme changes
    if (typeof window !== 'undefined') {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

      const handleChange = () => {
        if (currentTheme.value === 'system') {
          updateTheme();
        }
      };

      // Modern browsers
      if (mediaQuery.addEventListener) {
        mediaQuery.addEventListener('change', handleChange);
      } else {
        // Legacy support
        mediaQuery.addListener(handleChange);
      }
    }
  };

  return {
    currentTheme,
    isDark,
    setTheme,
    initTheme,
  };
}

// Initialize on module load for immediate theme application
if (typeof window !== 'undefined') {
  const saved = localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
  if (saved && ['light', 'dark', 'system'].includes(saved)) {
    currentTheme.value = saved;
  }
  updateTheme();
}
