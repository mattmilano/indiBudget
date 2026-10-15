import { createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import router from './router';
import { initApp } from './services/api';
import { useTheme } from './composables/useTheme';
import './styles.css';

// Initialize theme early to prevent flash
const { initTheme } = useTheme();
initTheme();

const app = createApp(App);
const pinia = createPinia();

app.use(pinia);
app.use(router);

// Initialize database BEFORE mounting the app
// This ensures all components can fetch data in their onMounted hooks
initApp()
  .then(() => {
    app.mount('#app');
  })
  .catch((err) => {
    console.error('Failed to initialize database:', err);
    // indiBudget declined to open the budget because another window already
    // has it. Say so plainly rather than mounting screens that cannot load.
    const refused =
      typeof err === 'object' && err !== null && (err as { kind?: string }).kind === 'refused';
    if (refused) {
      const root = document.getElementById('app');
      if (root) {
        const box = document.createElement('div');
        box.className = 'min-h-screen flex items-center justify-center p-8 bg-gray-50 dark:bg-gray-900';
        const text = document.createElement('p');
        text.className = 'max-w-xl text-center leading-relaxed text-gray-800 dark:text-gray-100';
        text.textContent = (err as { message: string }).message;
        box.appendChild(text);
        root.replaceChildren(box);
      }
      return;
    }
    // Mount anyway so user sees something, but show error state
    app.mount('#app');
  });
