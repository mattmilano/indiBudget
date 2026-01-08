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

initApp()
  .then(() => {
    console.log('Database initialized');
  })
  .catch((err) => {
    console.error('Failed to initialize database:', err);
  });

app.mount('#app');
