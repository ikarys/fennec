// Pure tests for the project file tree helpers.
import { describe, expect, it } from 'vitest'
import { ancestorsOf, buildFileTree, defaultCommitMessage, isMarkdown } from './fileTree'

describe('buildFileTree', () => {
  it('orders folders before files and sorts each group', () => {
    const tree = buildFileTree([
      { path: 'z.md', tracked: true },
      { path: 'a/b.md', tracked: true },
      { path: 'a/c.md', tracked: true },
      { path: 'b/a.md', tracked: true },
    ])

    expect(tree.map((node) => node.name)).toEqual(['a', 'b', 'z.md'])
    expect(tree[0].children).toEqual([
      { name: 'b.md', path: 'a/b.md', tracked: true, children: null },
      { name: 'c.md', path: 'a/c.md', tracked: true, children: null },
    ])
  })

  it('builds nested folders', () => {
    const tree = buildFileTree([{ path: 'a/b/c.md', tracked: true }])

    expect(tree[0].name).toBe('a')
    expect(tree[0].children?.[0].name).toBe('b')
    expect(tree[0].children?.[0].children?.[0].path).toBe('a/b/c.md')
  })

  it('marks a folder tracked when a descendant is tracked', () => {
    const tree = buildFileTree([
      { path: 'a/x.md', tracked: true },
      { path: 'a/y.md', tracked: false },
      { path: 'b/z.md', tracked: false },
    ])

    expect(tree[0].tracked).toBe(true)
    expect(tree[1].tracked).toBe(false)
  })
})

describe('ancestorsOf', () => {
  it('returns the folder paths leading to a file', () => {
    expect(ancestorsOf('a/b/c.md')).toEqual(['a', 'a/b'])
  })

  it('returns no ancestors for a root file', () => {
    expect(ancestorsOf('x.md')).toEqual([])
  })
})

describe('isMarkdown', () => {
  it('detects markdown files case-insensitively', () => {
    expect(isMarkdown('docs/a.md')).toBe(true)
    expect(isMarkdown('docs/A.MD')).toBe(true)
    expect(isMarkdown('a.markdown')).toBe(true)
    expect(isMarkdown('a.txt')).toBe(false)
  })
})

describe('defaultCommitMessage', () => {
  it('returns an empty message for no files', () => {
    expect(defaultCommitMessage([])).toBe('')
  })

  it('uses docs for one markdown file and chore for other files', () => {
    expect(defaultCommitMessage(['docs/a.md'])).toBe('docs: update a.md')
    expect(defaultCommitMessage(['src/b.ts'])).toBe('chore: update b.ts')
  })

  it('uses docs for several markdown files and chore otherwise', () => {
    expect(defaultCommitMessage(['docs/a.md', 'docs/b.md'])).toBe('docs: update 2 files')
    expect(defaultCommitMessage(['docs/a.md', 'src/b.ts'])).toBe('chore: update 2 files')
  })
})
