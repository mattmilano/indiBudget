<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useCategoriesStore } from '../stores';
import type { Category, CreateCategoryRequest } from '../types';
import ConfirmDialog from '../components/ConfirmDialog.vue';

const categoriesStore = useCategoriesStore();

const showAddModal = ref(false);
const showEditModal = ref(false);
const showDeleteConfirm = ref(false);
const categoryToDelete = ref<Category | null>(null);
const editingCategory = ref<Category | null>(null);
const activeTab = ref<'expense' | 'income'>('expense');

const newCategory = ref<CreateCategoryRequest>({
  name: '',
  category_type: 'expense',
  color: '#3b82f6',
  icon: undefined,
});

const editForm = ref({
  name: '',
  color: '#3b82f6',
  icon: '',
});

// Preset colors for easy selection
const presetColors = [
  '#ef4444', // red
  '#f97316', // orange
  '#f59e0b', // amber
  '#eab308', // yellow
  '#84cc16', // lime
  '#22c55e', // green
  '#14b8a6', // teal
  '#06b6d4', // cyan
  '#3b82f6', // blue
  '#6366f1', // indigo
  '#8b5cf6', // violet
  '#a855f7', // purple
  '#d946ef', // fuchsia
  '#ec4899', // pink
  '#f43f5e', // rose
  '#6b7280', // gray
];

// Filter categories by type
const displayedCategories = computed(() => {
  return categoriesStore.categories
    .filter(c => c.category_type === activeTab.value && c.is_active)
    .sort((a, b) => {
      // System categories first, then alphabetical
      if (a.is_system !== b.is_system) return a.is_system ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
});

// User-created categories only
const userCategories = computed(() => {
  return categoriesStore.categories.filter(c => !c.is_system && c.is_active);
});

async function handleSubmit() {
  if (!newCategory.value.name.trim()) {
    return;
  }
  try {
    newCategory.value.category_type = activeTab.value;
    await categoriesStore.createCategory(newCategory.value);
    showAddModal.value = false;
    resetForm();
  } catch (e) {
    console.error('Failed to create category:', e);
    alert('Failed to create category. Please try again.');
  }
}

function resetForm() {
  newCategory.value = {
    name: '',
    category_type: activeTab.value,
    color: '#3b82f6',
    icon: undefined,
  };
}

function openEditModal(category: Category) {
  if (category.is_system) {
    alert('System categories cannot be edited.');
    return;
  }
  editingCategory.value = category;
  editForm.value = {
    name: category.name,
    color: category.color,
    icon: category.icon || '',
  };
  showEditModal.value = true;
}

async function handleEditSubmit() {
  if (!editingCategory.value || !editForm.value.name.trim()) {
    return;
  }
  // Note: We don't have an update API yet, so we'll just show a message
  alert('Category updated successfully! (Note: Backend API for updates is pending)');
  showEditModal.value = false;
  editingCategory.value = null;
}

function confirmDelete(category: Category) {
  if (category.is_system) {
    alert('System categories cannot be deleted.');
    return;
  }
  categoryToDelete.value = category;
  showDeleteConfirm.value = true;
}

async function deleteCategory() {
  if (!categoryToDelete.value) return;
  // Note: We don't have a delete API yet, so we'll just show a message
  alert('Category deleted! (Note: Backend API for deletes is pending)');
  showDeleteConfirm.value = false;
  categoryToDelete.value = null;
}

onMounted(() => {
  categoriesStore.fetchCategories();
});
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Categories</h1>
      <button
        @click="showAddModal = true"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
        </svg>
        Add Category
      </button>
    </div>

    <!-- Summary Card -->
    <div class="bg-gradient-to-r from-purple-600 to-indigo-600 rounded-lg shadow-lg p-6 mb-6 text-white">
      <div class="flex flex-col md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-lg opacity-90 mb-1">Category Overview</h2>
          <p class="text-3xl font-bold">{{ categoriesStore.categories.length }} Categories</p>
        </div>
        <div class="flex gap-6 mt-4 md:mt-0">
          <div class="text-center">
            <p class="text-sm opacity-75">Expense</p>
            <p class="text-xl font-semibold">{{ categoriesStore.expenseCategories.length }}</p>
          </div>
          <div class="text-center">
            <p class="text-sm opacity-75">Income</p>
            <p class="text-xl font-semibold">{{ categoriesStore.incomeCategories.length }}</p>
          </div>
          <div class="text-center">
            <p class="text-sm opacity-75">Custom</p>
            <p class="text-xl font-semibold">{{ userCategories.length }}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Tab Selector -->
    <div class="flex gap-2 mb-6">
      <button
        @click="activeTab = 'expense'"
        :class="[
          'px-4 py-2 rounded-lg font-medium transition-colors',
          activeTab === 'expense'
            ? 'bg-red-600 text-white'
            : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
        ]"
      >
        Expense Categories
      </button>
      <button
        @click="activeTab = 'income'"
        :class="[
          'px-4 py-2 rounded-lg font-medium transition-colors',
          activeTab === 'income'
            ? 'bg-green-600 text-white'
            : 'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600'
        ]"
      >
        Income Categories
      </button>
    </div>

    <!-- Categories Grid -->
    <div v-if="displayedCategories.length > 0" class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
      <div
        v-for="category in displayedCategories"
        :key="category.id"
        class="bg-white dark:bg-gray-800 rounded-lg shadow p-4 hover:shadow-md transition-shadow"
      >
        <div class="flex items-start justify-between">
          <div class="flex items-center gap-3">
            <div
              class="w-10 h-10 rounded-full flex items-center justify-center"
              :style="{ backgroundColor: category.color + '20' }"
            >
              <div class="w-4 h-4 rounded-full" :style="{ backgroundColor: category.color }"></div>
            </div>
            <div>
              <h3 class="font-semibold text-gray-900 dark:text-white">{{ category.name }}</h3>
              <div class="flex items-center gap-2 mt-1">
                <span
                  v-if="category.is_system"
                  class="px-1.5 py-0.5 text-xs font-medium bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400 rounded"
                >
                  System
                </span>
                <span
                  v-else
                  class="px-1.5 py-0.5 text-xs font-medium bg-purple-100 dark:bg-purple-900/30 text-purple-600 dark:text-purple-400 rounded"
                >
                  Custom
                </span>
              </div>
            </div>
          </div>
          <div v-if="!category.is_system" class="flex gap-1">
            <button
              @click="openEditModal(category)"
              class="p-1.5 text-gray-400 hover:text-blue-600 transition-colors"
              title="Edit category"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
            </button>
            <button
              @click="confirmDelete(category)"
              class="p-1.5 text-gray-400 hover:text-red-600 transition-colors"
              title="Delete category"
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else class="bg-white dark:bg-gray-800 rounded-lg shadow p-12 text-center">
      <svg class="w-16 h-16 mx-auto text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
      </svg>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mt-4">No {{ activeTab }} categories</h3>
      <p class="text-gray-500 dark:text-gray-400 mt-2 mb-4">Create your first custom {{ activeTab }} category.</p>
      <button
        @click="showAddModal = true"
        class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
      >
        Add Category
      </button>
    </div>

    <!-- Add Category Modal -->
    <div
      v-if="showAddModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showAddModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
          <div class="p-2 bg-purple-100 dark:bg-purple-900 rounded-lg">
            <svg class="w-5 h-5 text-purple-600 dark:text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5c.512 0 1.024.195 1.414.586l7 7a2 2 0 010 2.828l-7 7a2 2 0 01-2.828 0l-7-7A1.994 1.994 0 013 12V7a4 4 0 014-4z" />
            </svg>
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Add {{ activeTab === 'expense' ? 'Expense' : 'Income' }} Category</h3>
        </div>
        <form @submit.prevent="handleSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Category Name <span class="text-red-500">*</span>
            </label>
            <input
              v-model="newCategory.name"
              type="text"
              required
              placeholder="e.g., Subscriptions"
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-purple-500 focus:border-transparent"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Color</label>
            <div class="grid grid-cols-8 gap-2">
              <button
                v-for="color in presetColors"
                :key="color"
                type="button"
                @click="newCategory.color = color"
                :class="[
                  'w-8 h-8 rounded-full border-2 transition-all',
                  newCategory.color === color
                    ? 'border-gray-900 dark:border-white scale-110'
                    : 'border-transparent hover:scale-105'
                ]"
                :style="{ backgroundColor: color }"
              ></button>
            </div>
            <div class="mt-3 flex items-center gap-2">
              <label class="text-sm text-gray-500">Custom:</label>
              <input
                v-model="newCategory.color"
                type="color"
                class="w-8 h-8 rounded cursor-pointer"
              />
              <span class="text-sm text-gray-500">{{ newCategory.color }}</span>
            </div>
          </div>
          <div class="flex justify-end gap-3 pt-4">
            <button
              type="button"
              @click="showAddModal = false"
              class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              class="px-4 py-2 bg-purple-600 text-white rounded-lg hover:bg-purple-700 transition-colors"
            >
              Add Category
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Edit Category Modal -->
    <div
      v-if="showEditModal && editingCategory"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showEditModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center gap-3">
          <div class="p-2 bg-blue-100 dark:bg-blue-900 rounded-lg">
            <svg class="w-5 h-5 text-blue-600 dark:text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
            </svg>
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Edit Category</h3>
        </div>
        <form @submit.prevent="handleEditSubmit" class="p-4 space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Category Name <span class="text-red-500">*</span>
            </label>
            <input
              v-model="editForm.name"
              type="text"
              required
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Color</label>
            <div class="grid grid-cols-8 gap-2">
              <button
                v-for="color in presetColors"
                :key="color"
                type="button"
                @click="editForm.color = color"
                :class="[
                  'w-8 h-8 rounded-full border-2 transition-all',
                  editForm.color === color
                    ? 'border-gray-900 dark:border-white scale-110'
                    : 'border-transparent hover:scale-105'
                ]"
                :style="{ backgroundColor: color }"
              ></button>
            </div>
            <div class="mt-3 flex items-center gap-2">
              <label class="text-sm text-gray-500">Custom:</label>
              <input
                v-model="editForm.color"
                type="color"
                class="w-8 h-8 rounded cursor-pointer"
              />
              <span class="text-sm text-gray-500">{{ editForm.color }}</span>
            </div>
          </div>
          <div class="flex justify-end gap-3 pt-4">
            <button
              type="button"
              @click="showEditModal = false"
              class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
            >
              Save Changes
            </button>
          </div>
        </form>
      </div>
    </div>

    <!-- Delete Confirmation -->
    <ConfirmDialog
      :show="showDeleteConfirm"
      title="Delete Category"
      :message="`Are you sure you want to delete '${categoryToDelete?.name}'? Transactions using this category will become uncategorized.`"
      confirm-text="Delete"
      cancel-text="Cancel"
      variant="danger"
      @confirm="deleteCategory"
      @cancel="showDeleteConfirm = false"
      @update:show="showDeleteConfirm = $event"
    />
  </div>
</template>
