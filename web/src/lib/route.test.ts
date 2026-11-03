import {
  describe,
  it,
  expect,
  vi,
  beforeEach,
  afterEach,
} from 'vitest';
import {
  parseRoute,
  tabPath,
  tabIdForRoute,
  splitPath,
  parsePaneToken,
  paneToken,
  PAGE_TABS,
  loadTabs,
  loadActiveTab,
  TABS_KEY,
  ACTIVE_TAB_KEY,
} from './route';

beforeEach(() => {
  const store = new Map<string, string>();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => {
      return store.get(key) ?? null;
    },
    setItem: (key: string, value: string) => {
      store.set(key, value);
    },
    removeItem: (key: string) => {
      store.delete(key);
    },
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('parseRoute', () => {
  it('parses empty hash as home', () => {
    expect(parseRoute('')).toEqual({ name: 'home' });
  });

  it('parses root hash as home', () => {
    expect(parseRoute('#/')).toEqual({ name: 'home' });
  });

  it('parses settings route with null tab', () => {
    expect(parseRoute('#/settings')).toEqual({
      name: 'settings',
      tab: null,
    });
  });

  it('parses settings route with tab', () => {
    expect(parseRoute('#/settings/rules')).toEqual({
      name: 'settings',
      tab: 'rules',
    });
  });

  it('parses insight route', () => {
    expect(parseRoute('#/insight')).toEqual({ name: 'insight' });
  });

  it('parses backlog route', () => {
    expect(parseRoute('#/backlog')).toEqual({ name: 'backlog' });
  });

  it('decodes project id with slash', () => {
    expect(parseRoute('#/project/a%2Fb/risks')).toEqual({
      name: 'project',
      projectId: 'a/b',
      tab: 'risks',
      sub: null,
    });
  });

  it('parses project route with sub', () => {
    expect(parseRoute('#/project/x/map/foo')).toEqual({
      name: 'project',
      projectId: 'x',
      tab: 'map',
      sub: 'foo',
    });
  });

  it('parses workflows route without ids', () => {
    expect(parseRoute('#/workflows')).toEqual({
      name: 'workflows',
      projectId: null,
      workflowId: null,
      isNew: false,
    });
  });

  it('parses workflows route with ids', () => {
    expect(parseRoute('#/workflows/p1/w1')).toEqual({
      name: 'workflows',
      projectId: 'p1',
      workflowId: 'w1',
      isNew: false,
    });
  });

  it('parses workflows route with new flag', () => {
    expect(parseRoute('#/workflows/p1/w1?new')).toEqual({
      name: 'workflows',
      projectId: 'p1',
      workflowId: 'w1',
      isNew: true,
    });
  });

  it('parses split route with left pane', () => {
    expect(parseRoute('#/split/s1/-')).toEqual({
      name: 'split',
      left: 's1',
      right: null,
    });
  });

  it('parses split route without panes', () => {
    expect(parseRoute('#/split')).toEqual({
      name: 'split',
      left: null,
      right: null,
    });
  });

  it('parses session route', () => {
    expect(parseRoute('#/s/abc')).toEqual({
      name: 'session',
      id: 'abc',
    });
  });

  it('parses new session route from encoded id', () => {
    expect(parseRoute('#/s/new%3Ap1')).toEqual({
      name: 'new',
      id: 'p1',
      testWorkflowId: null,
      frameIdeaId: null,
      fromSessionId: null,
    });
  });

  it('parses new route with query params', () => {
    expect(parseRoute('#/new/p1?test=w1&frame=i1&from=s0')).toEqual({
      name: 'new',
      id: 'p1',
      testWorkflowId: 'w1',
      frameIdeaId: 'i1',
      fromSessionId: 's0',
    });
  });

  it('parses new route without query params', () => {
    expect(parseRoute('#/new/p1')).toEqual({
      name: 'new',
      id: 'p1',
      testWorkflowId: null,
      frameIdeaId: null,
      fromSessionId: null,
    });
  });
});

describe('tabPath', () => {
  it('builds session path', () => {
    expect(tabPath('abc')).toBe('#/s/abc');
  });

  it('builds new path', () => {
    expect(tabPath('new:p1')).toBe('#/new/p1');
  });

  it('builds insight page path', () => {
    expect(tabPath('page:insight')).toBe('#/insight');
  });

  it('builds home path for unknown page', () => {
    expect(tabPath('page:unknown')).toBe('#/');
  });

  it('encodes slash in session id', () => {
    expect(tabPath('a/b')).toBe('#/s/a%2Fb');
  });
});

describe('tabIdForRoute', () => {
  it('returns session id for session route', () => {
    expect(tabIdForRoute({ name: 'session', id: 'abc' })).toBe('abc');
  });

  it('returns new id for new route', () => {
    expect(
      tabIdForRoute({
        name: 'new',
        id: 'p1',
        testWorkflowId: null,
        frameIdeaId: null,
        fromSessionId: null,
      }),
    ).toBe('new:p1');
  });

  it('returns settings page id', () => {
    expect(tabIdForRoute({ name: 'settings', tab: null })).toBe('page:settings');
  });

  it('returns insight page id', () => {
    expect(tabIdForRoute({ name: 'insight' })).toBe('page:insight');
  });

  it('returns backlog page id', () => {
    expect(tabIdForRoute({ name: 'backlog' })).toBe('page:backlog');
  });

  it('returns workflows page id', () => {
    expect(
      tabIdForRoute({
        name: 'workflows',
        projectId: null,
        workflowId: null,
        isNew: false,
      }),
    ).toBe('page:workflows');
  });

  it('returns null for home route', () => {
    expect(tabIdForRoute({ name: 'home' })).toBeNull();
  });

  it('returns null for project route', () => {
    expect(
      tabIdForRoute({
        name: 'project',
        projectId: 'p1',
        tab: null,
        sub: null,
      }),
    ).toBeNull();
  });

  it('returns null for split route', () => {
    expect(
      tabIdForRoute({
        name: 'split',
        left: null,
        right: null,
      }),
    ).toBeNull();
  });
});

describe('splitPath and parsePaneToken', () => {
  it('builds split path with left pane', () => {
    expect(splitPath('s1', null)).toBe('#/split/s1/-');
  });

  it('encodes new pane token in split path', () => {
    expect(splitPath(null, 'new:p1')).toBe('#/split/-/new%3Ap1');
  });

  it('returns null for dash pane token', () => {
    expect(parsePaneToken('-')).toBeNull();
  });

  it('returns null for undefined pane token', () => {
    expect(parsePaneToken(undefined)).toBeNull();
  });

  it('round trips new pane token', () => {
    expect(parsePaneToken(paneToken('new:p1'))).toBe('new:p1');
  });
});

describe('PAGE_TABS', () => {
  it('matches tab path for every page tab', () => {
    for (const entry of PAGE_TABS) {
      expect(entry.path).toBe(tabPath('page:' + entry.page));
    }
  });
});

describe('loadTabs', () => {
  it('returns empty array when key is missing', () => {
    expect(loadTabs()).toEqual([]);
  });

  it('keeps only plain string tabs', () => {
    localStorage.setItem(TABS_KEY, JSON.stringify(['abc', 1, 'page:x']));
    expect(loadTabs()).toEqual(['abc']);
  });

  it('returns empty array for invalid json', () => {
    localStorage.setItem(TABS_KEY, 'not json');
    expect(loadTabs()).toEqual([]);
  });

  it('returns empty array when local storage is missing', () => {
    vi.unstubAllGlobals();
    expect(loadTabs()).toEqual([]);
  });
});

describe('loadActiveTab', () => {
  it('returns null when key is missing', () => {
    expect(loadActiveTab()).toBeNull();
  });

  it('returns stored tab id', () => {
    localStorage.setItem(ACTIVE_TAB_KEY, 'abc');
    expect(loadActiveTab()).toBe('abc');
  });

  it('returns null for page tab', () => {
    localStorage.setItem(ACTIVE_TAB_KEY, 'page:settings');
    expect(loadActiveTab()).toBeNull();
  });

  it('returns null when local storage is missing', () => {
    vi.unstubAllGlobals();
    expect(loadActiveTab()).toBeNull();
  });
});