import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { CalendarEvent, UpcomingRecurring } from '../types';
import * as api from '../services/api';

export const useCalendarStore = defineStore('calendar', () => {
  const events = ref<CalendarEvent[]>([]);
  const upcomingRecurring = ref<UpcomingRecurring[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const currentDate = ref(new Date());

  const eventsByDate = computed(() => {
    const grouped: Record<string, CalendarEvent[]> = {};
    for (const event of events.value) {
      if (!grouped[event.date]) {
        grouped[event.date] = [];
      }
      grouped[event.date].push(event);
    }
    return grouped;
  });

  const dailyTotals = computed(() => {
    const totals: Record<string, { income: number; expenses: number }> = {};
    for (const event of events.value) {
      if (!totals[event.date]) {
        totals[event.date] = { income: 0, expenses: 0 };
      }
      const amount = parseFloat(event.amount) || 0;
      if (event.transaction_type === 'income') {
        totals[event.date].income += amount;
      } else if (event.transaction_type === 'expense') {
        totals[event.date].expenses += amount;
      }
    }
    return totals;
  });

  const upcomingBills = computed(() => {
    return upcomingRecurring.value.filter(r => r.recurring.transaction_type === 'expense');
  });

  const upcomingIncome = computed(() => {
    return upcomingRecurring.value.filter(r => r.recurring.transaction_type === 'income');
  });

  async function fetchCalendarEvents(startDate: string, endDate: string) {
    loading.value = true;
    error.value = null;
    try {
      events.value = await api.getCalendarEvents(startDate, endDate);
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  async function fetchUpcomingRecurring(days: number = 30) {
    loading.value = true;
    error.value = null;
    try {
      upcomingRecurring.value = await api.getUpcomingRecurring(days);
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }

  function setCurrentDate(date: Date) {
    currentDate.value = date;
  }

  function getEventsForDate(date: string): CalendarEvent[] {
    return eventsByDate.value[date] || [];
  }

  return {
    events,
    upcomingRecurring,
    loading,
    error,
    currentDate,
    eventsByDate,
    dailyTotals,
    upcomingBills,
    upcomingIncome,
    fetchCalendarEvents,
    fetchUpcomingRecurring,
    setCurrentDate,
    getEventsForDate,
  };
});
