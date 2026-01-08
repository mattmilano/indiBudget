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
    console.log('Database initialized');
    app.mount('#app');
  })
  .catch((err) => {
    console.error('Failed to initialize database:', err);
    // Mount anyway so user sees something, but show error state
    app.mount('#app');
  });
