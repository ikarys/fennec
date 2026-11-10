// The project file tree: paths grouped into folders, plus small path helpers for the Files tab.
export interface FileTreeNode {
  name: string
  path: string
  tracked: boolean
  children: FileTreeNode[] | null
}

export function buildFileTree(files: { path: string; tracked: boolean }[]): FileTreeNode[] {
  const root: FileTreeNode[] = []
  const folders = new Map<string, FileTreeNode>()

  const ensureFolder = (path: string): FileTreeNode => {
    const existing = folders.get(path)
    if (existing) return existing

    const name = path.slice(path.lastIndexOf('/') + 1)
    const node: FileTreeNode = { name, path, tracked: false, children: [] }
    folders.set(path, node)

    const slash = path.lastIndexOf('/')
    const parentPath = slash < 0 ? '' : path.slice(0, slash)
    if (parentPath) {
      ensureFolder(parentPath).children?.push(node)
    } else {
      root.push(node)
    }

    return node
  }

  for (const file of files) {
    const slash = file.path.lastIndexOf('/')
    const name = file.path.slice(slash + 1)
    const parentPath = slash < 0 ? '' : file.path.slice(0, slash)
    const siblings = parentPath ? ensureFolder(parentPath).children ?? [] : root

    siblings.push({
      name,
      path: file.path,
      tracked: file.tracked,
      children: null,
    })
  }

  const sortNodes = (nodes: FileTreeNode[]) => {
    nodes.sort((a, b) => {
      const aFolder = a.children !== null
      const bFolder = b.children !== null
      if (aFolder !== bFolder) return aFolder ? -1 : 1
      return a.name.localeCompare(b.name)
    })

    for (const node of nodes) {
      if (node.children) sortNodes(node.children)
    }
  }
  sortNodes(root)

  const propagateTracked = (node: FileTreeNode): boolean => {
    if (!node.children) return node.tracked

    let tracked = node.tracked
    for (const child of node.children) {
      if (propagateTracked(child)) tracked = true
    }
    node.tracked = tracked
    return tracked
  }
  root.forEach(propagateTracked)

  return root
}

export function ancestorsOf(path: string): string[] {
  const parts = path.split('/')
  return parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join('/'))
}

export function isMarkdown(path: string): boolean {
  const name = path.slice(path.lastIndexOf('/') + 1).toLowerCase()
  return name.endsWith('.md') || name.endsWith('.markdown')
}

export function defaultCommitMessage(path: string): string {
  const name = path.slice(path.lastIndexOf('/') + 1)
  return isMarkdown(path) ? `docs: update ${name}` : `chore: update ${name}`
}
