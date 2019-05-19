const fundb = require('./SGDB')


// VALIDAR INPUTS

exports.createDB = (name) => {
    if (!name) return console.log('Invalid name!')

    fundb.createDB(name)

    console.log(`Created Database ${name} with success!`)
}

exports.createTable = (table) => {
    if (!table) return console.log('Input invalid!')

    fundb.createTable(table)
}

exports.listDatabases = () => {
    const dbs = fundb.listDatabases()

    if (!dbs.length)
        return console.log('No databases!\n')

    console.log('All databases\n')
    dbs.map(db => console.log(` 🛢\  >> ${db}\n`))
}

exports.find = (table, index) => {
    fundb.find(table, index)
}

exports.insert = (table, dados) => {
    fundb.insert(table, dados)
}

exports.setDatabase = (dbName) => {
    const db = fundb.setCurrentDatabase(dbName)

    if (db)
        return db

    console.log('Database not exists')
    return false
}

exports.listTables = () => {
    const dbName = fundb.getCurrentDatabase()
    if (!dbName) return console.log('Select a database before!')


    const tables = fundb.listTables()
    if (!tables.length)
        return console.log('\nNo tables!\n')

    console.log(`All table from ${dbName}\n`)

    tables.map(table => console.log(`  📂\ >> ${table}\n`))
}