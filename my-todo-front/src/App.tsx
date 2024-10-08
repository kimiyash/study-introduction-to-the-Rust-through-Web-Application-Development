import { useEffect, useState, FC } from 'react'
import 'modern-css-reset'
import { ThemeProvider, createTheme } from '@mui/material/styles'
import { Box, Stack, Typography } from '@mui/material'
import { NewTodoPayload, Todo } from './types/todo'
import TodoList from './components/TodoList'
import TodoForm from './components/TodoForm'
import {
  addTodoItem,
  deleteTodoItem,
  getTodoItems,
  updateTodoItem
} from './lib/api/todo'

const TodoApp: FC = () => {
  const [todos, setTodos] = useState<Todo[]>([])

  const onSubmit = async (payload: NewTodoPayload) => {
    if (!payload.text) return

    await addTodoItem(payload)
    const todos = await getTodoItems()
    setTodos(todos)
  }

  const onUpdate = async (updateTodo: Todo) => {
    await updateTodoItem(updateTodo)
    const todos = await getTodoItems()
    setTodos(todos)
  }

  const onDelete = async (id: number) => {
    await deleteTodoItem(id)
    const todos = await getTodoItems()
    setTodos(todos)
  }

  useEffect(() => {
      ;(async() => {
        const todos = await getTodoItems()
        setTodos(todos)
      }) ()
  }, [])

  return (
    <>
      <Box
        sx={{
          backgroudColor: 'white',
          borderBottom: '1px solid gray',
          display: 'flex',
          alignItems: 'center',
          postiton: 'flxed',
          top: 0,
          p: 2,
          width: '100%',
          height: 80,
          zIndex: 3,
        }}
      >
        <Typography variant='h1'>Todo App</Typography>
      </Box>
      <Box
        sx={{
          display: 'flex',
          justifyContent: 'center',
          p: 1,
          mt: 3,
        }}
      >
        <Box maxWidth={700} width="100%">
          <Stack spacing={5}>
            <TodoForm onSubmit={onSubmit} />
            <TodoList todos={todos} onUpdate={onUpdate} onDelete={onDelete} />
          </Stack>
        </Box>
      </Box>
    </>
  )
}

const theme = createTheme({
  typography: {
    h1: {
      fontSize: 30,
    },
    h2: {
      fontSize: 20,
    },
  },
})

const App: FC = () => {
  return (
    <ThemeProvider theme={theme}>
      <TodoApp />
    </ThemeProvider>
  )
}

export default App