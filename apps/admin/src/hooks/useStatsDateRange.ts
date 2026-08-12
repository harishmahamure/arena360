import { subDays } from 'date-fns';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { endOfTodayIST, now, startOfTodayIST } from '../utils/date';

export type StatsDatePreset = 'today' | 'last7' | 'mtd';

function calendarDateInIST(date: Date): string {
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Asia/Kolkata',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(date);
}

function startOfCalendarDayIST(yyyyMmDd: string): string {
  return `${yyyyMmDd}T00:00:00+05:30`;
}

function endOfCalendarDayIST(yyyyMmDd: string): string {
  return `${yyyyMmDd}T23:59:59+05:30`;
}

function defaultStart(): string {
  return calendarDateInIST(startOfTodayIST());
}

function defaultEnd(): string {
  return calendarDateInIST(endOfTodayIST());
}

function presetRange(preset: StatsDatePreset): { start: string; end: string } {
  const current = now();
  const end = defaultEnd();
  switch (preset) {
    case 'today':
      return { start: defaultStart(), end };
    case 'last7':
      return { start: calendarDateInIST(subDays(current, 6)), end };
    case 'mtd':
      return { start: `${end.slice(0, 8)}01`, end };
  }
}

/**
 * URL-persisted stats date range with draft edits and Apply.
 * Draft changes do not hit the API until `apply()` commits them to the URL.
 */
export function useStatsDateRange() {
  const [searchParams, setSearchParams] = useSearchParams();

  const appliedStart = searchParams.get('startDate') || defaultStart();
  const appliedEnd = searchParams.get('endDate') || defaultEnd();
  const appliedCompare = searchParams.get('compare') !== 'false';

  const [draftStart, setDraftStart] = useState(appliedStart);
  const [draftEnd, setDraftEnd] = useState(appliedEnd);
  const [draftCompare, setDraftCompare] = useState(appliedCompare);

  // Keep draft in sync when URL changes externally (e.g. browser back)
  useEffect(() => {
    setDraftStart(appliedStart);
    setDraftEnd(appliedEnd);
    setDraftCompare(appliedCompare);
  }, [appliedStart, appliedEnd, appliedCompare]);

  const apiFilters = useMemo(
    () => ({
      startDate: startOfCalendarDayIST(appliedStart),
      endDate: endOfCalendarDayIST(appliedEnd),
      compare: appliedCompare,
    }),
    [appliedStart, appliedEnd, appliedCompare],
  );

  const isDirty =
    draftStart !== appliedStart || draftEnd !== appliedEnd || draftCompare !== appliedCompare;

  const setRange = useCallback((nextStart: string, nextEnd: string) => {
    setDraftStart(nextStart);
    setDraftEnd(nextEnd);
  }, []);

  const setCompare = useCallback((value: boolean) => {
    setDraftCompare(value);
  }, []);

  const applyPreset = useCallback((preset: StatsDatePreset) => {
    const { start, end } = presetRange(preset);
    setDraftStart(start);
    setDraftEnd(end);
  }, []);

  const apply = useCallback(() => {
    const next = new URLSearchParams(searchParams);
    next.set('startDate', draftStart);
    next.set('endDate', draftEnd);
    if (draftCompare) {
      next.delete('compare');
    } else {
      next.set('compare', 'false');
    }
    setSearchParams(next, { replace: true });
  }, [searchParams, setSearchParams, draftStart, draftEnd, draftCompare]);

  return {
    /** Draft values bound to toolbar inputs */
    startDate: draftStart,
    endDate: draftEnd,
    compare: draftCompare,
    /** Applied values used for API queries */
    apiFilters,
    /** Applied compare (for rendering % chips from fetched data) */
    appliedCompare,
    isDirty,
    setRange,
    setCompare,
    applyPreset,
    apply,
  };
}
