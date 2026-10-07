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
    // A second window on the same budget is refused by the lock beside the
    // database. Say so plainly rather than mounting a screen that cannot load.
    if (String(err).includes('already open in another window')) {
      const root = document.getElementById('app');
      if (root) {
        const box = document.createElement('div');
        box.className = 'min-h-screen flex items-center justify-center p-8 bg-gray-50 dark:bg-gray-900';
        const text = document.createElement('p');
        text.className = 'max-w-md text-center text-gray-800 dark:text-gray-100';
        text.textContent = String(err);
        box.appendChild(text);
        root.replaceChildren(box);
      }
      return;
    }
    // Mount anyway so user sees something, but show error state
    app.mount('#app');
  });
