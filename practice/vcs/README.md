# Система контроля версий

VCS разделена на 2 основных части: одна реализует непосредственно CLI с валидацией пользовательского ввода,
вторая — библиотека, абстрагирующая пользовательский интерфейс от работы с локальным и удалённым хранилищем.

Репозиторий — это дерево из коммитов. Каждый коммит содержит ссылку на дерево, которое хранит в себе
информацию о содержимом объектов/файлов в репозитории. Объекты хранятся в сжатом виде и адресуются по хэшу
содержимого. Хранилище по хэшу можно абстрагировать и реализовать как в виде файлов в локальной
ФС, так и в удалённом хранилище.

## Диаграмма классов
```mermaid
classDiagram
    namespace VCSCore {
        class Repository {
            +name: string
            +remotes: List~Remote~

            +checkout(ref: revisionReference)
            +createBranch(name: string)
            +deleteBranch(name: string)
            +mergeBranch(name: string)

            +log()

            +addToIndex(path: string)
            +removeFile(path: string)

            +addRemote(name: string, URL: string)
            +changeRemote(name: string, newName: string, newURL: string)

            +pushToRemote(remoteName: string)
            +fetchFromRemote(remoteName: string)
            +pullFromRemote(remoteName: string)
        }

        class Revision {
            sha: SHA-512
            commitMessage: string
            commitDate: Timestamp
            parent: Revision
        }

        class Author {
            +name: string
            +email: string
        }

        class Snapshot {
            FileTree: Map~fullPath: string, fileHash: SHA-512~
        }

        class Remote {
            +name: string
            +URL: string

            +setName(name: string)
            +setURL(URL: string)
        }

        class Index {
            indexedFiles Map~filePath: string, fileHash: SHA-512~
        }

        class Branch {
            +name: string
        }
    }

    namespace StorageTools {
        class Storage {
            <<interface>>
            +get(fileHash: SHA-512)
            +put(fileHash: SHA-512, fileContent: bytes)
        }

        class LocalFS
        class SQLite
        class S3
        class MongoDB
    }

    namespace FileTools {
        class DiffEngine {
            <<interface>>
            computeDiff(content1: bytes, content2: bytes)
        }

        class MergeEngine {
        }

        class MergeAlgorithm {
            <<interface>>
            +performMerge(theirs: bytes, ours: bytes, original: bytes, fullPath: string)
        }

        class ThreeWayMerge {
        }

        class CustomMerge {

        }
    }

    namespace RemoteTools {
        class Server {
            -handleClone() Repository
            -handleFetch(branchName: string)
            -handlePush(targetBranch: string, revision: Revision)
        }

        class Client {
            +clone(URL: string, destinationPath: string) Repository
            +fetch(remote: string)
            +push(remote: string, targetBranch: string, revision: Revision)
        }
    }

    namespace UI {
        class CLI {
            -parseCommand(input: string)
            -performAction(cmd: string, args: List<string>)
            +startServer(addr: string, port: uint)
        }
    }

    Repository "1" o-- "1" Revision : activeRevision
    Repository "1" o-- "1" Index : index
    Repository "1" o-- "1" Storage: blobStorage
    Repository "1" o-- "*" Remote: remotes
    Repository "1" o-- "1" Remote: origin

    Revision "*" o-- "1" Author: commitAuthor
    Revision "1" o-- "1" Snapshot: snapshot

    Repository --> MergeEngine
    Repository "1" o-- "*" Branch: branches

    Branch "*" o-- "1" Revision: revision

    Storage <|-- LocalFS
    Storage <|-- SQLite
    Storage <|-- S3
    Storage <|-- MongoDB

    MergeAlgorithm <|-- ThreeWayMerge
    MergeAlgorithm <|-- CustomMerge

    MergeEngine o-- DiffEngine: diffEngine
    MergeEngine o-- MergeAlgorithm: mergeAlgorithmImpl

    CLI --> Client
    CLI --> Repository
    CLI --> Server
end
```


## Диаграммма компонентов
```mermaid
flowchart LR

subgraph UI
    CLI[CLI]
end

subgraph VCSCore
    Repository[Repository]
    Revision[Revision]
    Branch[Branch]
    Index[Index]
    Snapshot[Snapshot]
    Remote[Remote]
end

subgraph StorageTools
    Storage[[Storage interface]]
    LocalFS[LocalFS]
    SQLite[SQLite]
    S3[S3]
    MongoDB[MongoDB]
end

subgraph FileTools
    MergeEngine[MergeEngine]
    DiffEngine[[DiffEngine interface]]
    MergeAlgorithm[[MergeAlgorithm interface]]
    ThreeWayMerge[ThreeWayMerge]
    CustomMerge[CustomMerge]
end

subgraph VCSLib
    subgraph RemoteTools
        Client[Client]
        Server[Server]
    end

    RepositoryTools
end

CLI --> RepositoryTools
CLI --> Client
CLI --> Server
RepositoryTools --> Repository

Repository --> Revision
Repository --> Branch
Repository --> Index
Repository --> Snapshot
Repository --> Remote
Repository --> Storage
Repository --> MergeEngine

LocalFS -. implements .-> Storage
SQLite -. implements .-> Storage
S3 -. implements .-> Storage
MongoDB -. implements .-> Storage

MergeEngine --> DiffEngine
MergeEngine --> MergeAlgorithm
ThreeWayMerge -. implements .-> MergeAlgorithm
CustomMerge -. implements .-> MergeAlgorithm

Client --> Server
Client --> Repository
Server --> Repository
```
