import chokidar from 'chokidar'
import { Logger } from 'winston'
import path from 'path'

export interface FileChange {
  type: 'add' | 'change' | 'unlink'
  path: string
  relativePath: string
  content?: string
}

export class DirectoryWatcher {
  private watcher: chokidar.FSWatcher
  private baseDir: string

  constructor(
    directory: string,
    private onChange: (change: FileChange) => void,
    private logger: Logger
  ) {
    this.baseDir = path.resolve(directory)
    this.watcher = chokidar.watch(directory, {
      ignored: /(^|[\/\\])\../, // ignore dotfiles
      persistent: true,
      ignoreInitial: false
    })

    this.setupWatchers()
  }

  private setupWatchers() {
    this.watcher
      .on('add', (path) => this.handleChange('add', path))
      .on('change', (path) => this.handleChange('change', path))
      .on('unlink', (path) => this.handleChange('unlink', path))
      .on('error', (error) => this.logger.error('Error watching files:', error))
  }

  private handleChange(type: FileChange['type'], filePath: string) {
    const relativePath = path.relative(this.baseDir, filePath)
    this.logger.debug(`File ${type}: ${relativePath}`)
    this.onChange({
      type,
      path: filePath,
      relativePath
    })
  }

  public close() {
    this.watcher.close()
  }
} 