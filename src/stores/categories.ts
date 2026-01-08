import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { Category, CreateCategoryRequest } from '../types';
import * as api from '../services/api';

export const useCategoriesStore = defineStore('categories', () => {
  const categories = ref<Category[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const incomeCategories = computed(() =>
    categories.value.filter(c => c.category_type === 'income' && c.is_active)
  );

  const expenseCategories = computed(() =>
    categories.value.filter(c => c.category_type === 'expense' && c.is_active)
  );

  const categoriesById = computed(() => {
    return categories.value.reduce((map, cat) => {
      map[cat.id] = cat;
      return map;
    }, {} as Record<string, Category>);
  });

  async function fetchCategories() {
    loading.value = true;
    error.value = null;
    try {
      categories.value = await api.getCategories();
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function createCategory(request: CreateCategoryRequest) {
    loading.value = true;
    error.value = null;
    try {
      const category = await api.createCategory(request);
      categories.value.push(category);
      return category;
    } catch (e) {
      error.value = String(e);
      throw e;
    } finally {
      loading.value = false;
    }
  }

  function getCategoryById(id: string): Category | undefined {
    return categoriesById.value[id];
  }

  return {
    categories,
    loading,
    error,
    incomeCategories,
    expenseCategories,
    categoriesById,
    fetchCategories,
    createCategory,
    getCategoryById,
  };
});
